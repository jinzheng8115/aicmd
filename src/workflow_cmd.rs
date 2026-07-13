use crate::confirm_cmd::{confirm_high_risk, effective_workflow_risk, read_action};
use crate::plan_cmd::{
    WorkflowConditionResult, WorkflowFailurePolicy, WorkflowPlan, WorkflowRisk, WorkflowStep,
    WorkflowStepKind,
};
use crate::utils::localized;

use anyhow::Result;
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub exit_code: i32,
    pub termination: String,
    pub stdout: String,
    pub stderr: String,
}

impl StepResult {
    pub fn exited(step_id: &str, exit_code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            step_id: step_id.to_string(),
            status: if exit_code == 0 {
                StepStatus::Passed
            } else {
                StepStatus::Failed
            },
            exit_code,
            termination: "exited".to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedWorkflow {
    pub plan: WorkflowPlan,
    pub results: Vec<StepResult>,
    statuses: IndexMap<String, StepStatus>,
}

pub fn prepare_workflow(plan: WorkflowPlan, results: &[StepResult]) -> Result<PreparedWorkflow> {
    let mut workflow = PreparedWorkflow {
        statuses: plan
            .steps
            .iter()
            .map(|step| (step.id.clone(), StepStatus::Pending))
            .collect(),
        plan,
        results: Vec::new(),
    };

    for result in results {
        workflow.record(result.clone())?;
    }
    Ok(workflow)
}

impl PreparedWorkflow {
    pub fn status_of(&self, step_id: &str) -> StepStatus {
        *self
            .statuses
            .get(step_id)
            .expect("workflow status exists for every planned step")
    }

    pub fn needs_confirmation(&self) -> bool {
        self.confirmation_risk().is_some()
    }

    fn confirmation_risk(&self) -> Option<WorkflowRisk> {
        self.next_step()
            .map(|step| effective_workflow_risk(&step.command, step.risk))
            .filter(|risk| *risk > WorkflowRisk::ReadOnly)
    }

    pub fn next_step(&self) -> Option<&WorkflowStep> {
        (!self.is_stopped()).then(|| {
            self.plan
                .steps
                .iter()
                .find(|step| self.status_of(&step.id) == StepStatus::Pending)
        })?
    }

    pub fn record(&mut self, result: StepResult) -> Result<()> {
        if self.is_stopped() {
            anyhow::bail!("workflow is stopped")
        }
        let next_step = self
            .next_step()
            .ok_or_else(|| anyhow::anyhow!("workflow has no eligible step"))?;
        if result.step_id != next_step.id {
            anyhow::bail!(
                "expected workflow step '{}', got '{}'",
                next_step.id,
                result.step_id
            )
        }
        let status = self
            .statuses
            .get_mut(&result.step_id)
            .ok_or_else(|| anyhow::anyhow!("unknown workflow step '{}'", result.step_id))?;
        *status = result.status;
        self.results.push(result);
        self.apply_conditions();
        Ok(())
    }

    pub fn is_stopped(&self) -> bool {
        self.results.iter().any(|result| {
            result.status == StepStatus::Cancelled
                || (result.status == StepStatus::Failed
                    && matches!(
                        self.step(&result.step_id).on_failure,
                        WorkflowFailurePolicy::Stop | WorkflowFailurePolicy::Repair
                    ))
        })
    }

    pub fn completed(&self) -> bool {
        !self.is_stopped()
            && self.plan.steps.iter().all(|step| {
                matches!(
                    self.status_of(&step.id),
                    StepStatus::Passed | StepStatus::Skipped
                )
            })
            && self.plan.steps.iter().any(|step| {
                step.kind == WorkflowStepKind::Verify
                    && self.status_of(&step.id) == StepStatus::Passed
            })
    }

    fn step(&self, step_id: &str) -> &WorkflowStep {
        self.plan
            .steps
            .iter()
            .find(|step| step.id == step_id)
            .expect("recorded result belongs to the workflow plan")
    }

    fn apply_conditions(&mut self) {
        for step in &self.plan.steps {
            let Some(condition) = &step.run_if else {
                continue;
            };
            let condition_matches = matches!(
                (self.status_of(&condition.step), condition.result),
                (StepStatus::Passed, WorkflowConditionResult::Passed)
                    | (StepStatus::Failed, WorkflowConditionResult::Failed)
            );
            if !condition_matches
                && matches!(
                    self.status_of(&condition.step),
                    StepStatus::Passed | StepStatus::Failed
                )
                && self.status_of(&step.id) == StepStatus::Pending
            {
                self.statuses.insert(step.id.clone(), StepStatus::Skipped);
            }
        }
    }
}

pub fn render_workflow_confirmation(prepared: &PreparedWorkflow) -> String {
    let mut lines = vec![
        format!(
            "{}: {}",
            localized("工作流确认", "Workflow confirmation"),
            prepared.plan.summary
        ),
        localized("已准备计划:", "Prepared plan:").to_string(),
    ];

    for step in &prepared.plan.steps {
        let status = prepared.status_of(&step.id);
        match status {
            StepStatus::Pending => lines.push(format!(
                "  [{}] {} ({}) : {}",
                localized("待执行", "pending"),
                step.id,
                workflow_risk_label(effective_workflow_risk(&step.command, step.risk)),
                step.command
            )),
            StepStatus::Skipped => lines.push(format!(
                "  [{}] {}",
                localized("已跳过", "skipped"),
                step.id
            )),
            StepStatus::Passed => {
                lines.push(format!("  [{}] {}", localized("已完成", "passed"), step.id))
            }
            StepStatus::Failed => {
                lines.push(format!("  [{}] {}", localized("已失败", "failed"), step.id))
            }
            StepStatus::Cancelled => lines.push(format!(
                "  [{}] {}",
                localized("已取消", "cancelled"),
                step.id
            )),
        }
    }
    lines.join("\n")
}

pub fn confirm_workflow(prepared: &PreparedWorkflow) -> Result<bool> {
    let Some(risk) = prepared.confirmation_risk() else {
        return Ok(true);
    };

    println!("{}", render_workflow_confirmation(prepared));
    if read_action(
        &['y', 'n'],
        'n',
        localized("执行此计划？ [y/N] ", "Execute this plan? [y/N] "),
    )? != 'y'
    {
        return Ok(false);
    }
    if risk == WorkflowRisk::Destructive
        && !confirm_high_risk(localized(
            "检测到破坏性步骤，确认继续？",
            "Destructive step detected. Continue?",
        ))?
    {
        return Ok(false);
    }
    Ok(true)
}

fn workflow_risk_label(risk: WorkflowRisk) -> &'static str {
    match risk {
        WorkflowRisk::ReadOnly => localized("只读", "read-only"),
        WorkflowRisk::ChangesFiles => localized("会修改文件", "changes files"),
        WorkflowRisk::ChangesSystem => localized("会修改系统或文件", "changes system"),
        WorkflowRisk::Destructive => localized("可能造成破坏", "destructive"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_cmd::{parse_execution_plan, WorkflowPlan};
    use std::{
        env,
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::{LazyLock, Mutex},
    };

    static LANGUAGE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    const THREE_STEP_WORKFLOW_JSON: &str = r#"{
      "mode":"workflow","command":"","query":"","problem":"","preflight":[],
      "summary":"Install tool",
      "steps":[
        {"id":"check","kind":"check","command":"command -v tool","risk":"read_only","on_failure":"continue"},
        {"id":"install","kind":"action","command":"brew install tool","risk":"changes_system","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
        {"id":"verify","kind":"verify","command":"tool --version","risk":"read_only","on_failure":"repair"}
      ]
    }"#;

    fn fixture_plan() -> WorkflowPlan {
        parse_execution_plan(THREE_STEP_WORKFLOW_JSON)
            .unwrap()
            .workflow()
            .unwrap()
    }

    fn destructive_fixture_plan() -> WorkflowPlan {
        parse_execution_plan(
            r#"{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Remove temporary files",
              "steps":[
                {"id":"check","kind":"check","command":"command -v tool","risk":"read_only","on_failure":"continue"},
                {"id":"remove","kind":"action","command":"rm -rf /tmp/aicmd-test","risk":"read_only","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
                {"id":"verify","kind":"verify","command":"tool --version","risk":"read_only","on_failure":"repair"}
              ]
            }"#,
        )
        .unwrap()
        .workflow()
        .unwrap()
    }

    struct TestLanguageConfig {
        original_config_file: Option<OsString>,
        path: PathBuf,
    }

    impl TestLanguageConfig {
        fn new(language: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "aicmd-workflow-language-{}.yaml",
                uuid::Uuid::new_v4()
            ));
            fs::write(&path, format!("language: {language}\n")).unwrap();
            let original_config_file = env::var_os("AICMD_CONFIG_FILE");
            env::set_var("AICMD_CONFIG_FILE", &path);
            Self {
                original_config_file,
                path,
            }
        }
    }

    impl Drop for TestLanguageConfig {
        fn drop(&mut self) {
            match &self.original_config_file {
                Some(path) => env::set_var("AICMD_CONFIG_FILE", path),
                None => env::remove_var("AICMD_CONFIG_FILE"),
            }
            let _ = fs::remove_file(&self.path);
        }
    }

    fn render_in_language(language: &str, prepared: &PreparedWorkflow) -> String {
        let _lock = LANGUAGE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _config = TestLanguageConfig::new(language);
        render_workflow_confirmation(prepared)
    }

    #[test]
    fn confirmation_after_failed_check_enables_risky_action() {
        let plan = fixture_plan();
        let results = vec![StepResult::exited("check", 1, "", "not found")];
        let prepared = prepare_workflow(plan, &results).unwrap();
        assert_eq!(prepared.status_of("install"), StepStatus::Pending);
        assert!(prepared.needs_confirmation());
    }

    #[test]
    fn no_confirmation_before_unresolved_check() {
        let prepared = prepare_workflow(fixture_plan(), &[]).unwrap();
        assert_eq!(prepared.next_step().unwrap().id, "check");
        assert!(!prepared.needs_confirmation());
    }

    #[test]
    fn confirmation_risk_tracks_only_the_currently_eligible_step() {
        let before_check = prepare_workflow(destructive_fixture_plan(), &[]).unwrap();
        assert_eq!(before_check.confirmation_risk(), None);

        let after_failed_check = prepare_workflow(
            destructive_fixture_plan(),
            &[StepResult::exited("check", 1, "", "not found")],
        )
        .unwrap();
        assert_eq!(
            after_failed_check.confirmation_risk(),
            Some(WorkflowRisk::Destructive)
        );

        let after_passed_check = prepare_workflow(
            destructive_fixture_plan(),
            &[StepResult::exited("check", 0, "/usr/bin/tool", "")],
        )
        .unwrap();
        assert_eq!(after_passed_check.confirmation_risk(), None);
    }

    #[test]
    fn passed_check_skips_conditioned_install() {
        let plan = fixture_plan();
        let results = vec![StepResult::exited("check", 0, "/usr/bin/tool", "")];
        let prepared = prepare_workflow(plan, &results).unwrap();
        assert_eq!(prepared.status_of("install"), StepStatus::Skipped);
        assert!(!prepared.needs_confirmation());
        assert_eq!(prepared.next_step().unwrap().id, "verify");
    }

    #[test]
    fn workflow_confirmation_shows_only_pending_changes_and_verification() {
        let prepared = prepare_workflow(
            fixture_plan(),
            &[StepResult::exited("check", 0, "/usr/bin/tool", "")],
        )
        .unwrap();
        let text = render_in_language("en", &prepared);
        assert!(text.contains("verify"));
        assert!(text.contains("skipped"));
        assert!(!text.contains("brew install tool"));
    }

    #[test]
    fn workflow_confirmation_uses_only_the_configured_language() {
        let prepared = prepare_workflow(fixture_plan(), &[]).unwrap();
        let chinese = render_in_language("zh", &prepared);
        assert!(chinese.contains("工作流确认"));
        assert!(!chinese.contains("Workflow confirmation"));
        assert!(!chinese.contains("[pending"));
        assert!(!chinese.contains("[skipped"));
        assert!(!chinese.contains("[passed"));

        let english = render_in_language("en", &prepared);
        assert!(english.contains("Workflow confirmation"));
        assert!(!english.contains("工作流确认"));
        assert!(!english.contains("待执行"));
        assert!(!english.contains("已跳过"));
        assert!(!english.contains("已完成"));
    }

    #[test]
    fn workflow_confirmation_skips_prompt_without_pending_modifications() {
        let prepared = prepare_workflow(
            fixture_plan(),
            &[StepResult::exited("check", 0, "/usr/bin/tool", "")],
        )
        .unwrap();
        assert!(confirm_workflow(&prepared).unwrap());
    }

    #[test]
    fn modification_failure_stops_later_modifications() {
        let mut workflow =
            prepare_workflow(fixture_plan(), &[StepResult::exited("check", 1, "", "")]).unwrap();
        workflow
            .record(StepResult::exited("install", 1, "", "failed"))
            .unwrap();
        assert!(workflow.is_stopped());
        assert!(workflow.next_step().is_none());
    }

    #[test]
    fn out_of_order_record_is_rejected() {
        let mut workflow = prepare_workflow(fixture_plan(), &[]).unwrap();
        assert!(workflow
            .record(StepResult::exited("install", 0, "", ""))
            .is_err());
    }

    #[test]
    fn skipped_step_record_is_rejected() {
        let mut workflow =
            prepare_workflow(fixture_plan(), &[StepResult::exited("check", 0, "", "")]).unwrap();
        assert!(workflow
            .record(StepResult::exited("install", 0, "", ""))
            .is_err());
    }

    #[test]
    fn record_after_stop_is_rejected() {
        let mut workflow =
            prepare_workflow(fixture_plan(), &[StepResult::exited("check", 1, "", "")]).unwrap();
        workflow
            .record(StepResult::exited("install", 1, "", "failed"))
            .unwrap();
        assert!(workflow
            .record(StepResult::exited("verify", 0, "ok", ""))
            .is_err());
    }

    #[test]
    fn cancelled_step_stops_later_work() {
        let mut workflow =
            prepare_workflow(fixture_plan(), &[StepResult::exited("check", 1, "", "")]).unwrap();
        workflow
            .record(StepResult {
                step_id: "install".to_string(),
                status: StepStatus::Cancelled,
                exit_code: 130,
                termination: "cancelled".to_string(),
                stdout: String::new(),
                stderr: String::new(),
            })
            .unwrap();
        assert_eq!(workflow.status_of("install"), StepStatus::Cancelled);
        assert!(workflow.is_stopped());
        assert!(workflow.next_step().is_none());
    }

    #[test]
    fn workflow_completes_only_after_verification_passes() {
        let mut workflow =
            prepare_workflow(fixture_plan(), &[StepResult::exited("check", 0, "", "")]).unwrap();
        assert!(!workflow.completed());
        workflow
            .record(StepResult::exited("verify", 0, "ok", ""))
            .unwrap();
        assert!(workflow.completed());
    }
}
