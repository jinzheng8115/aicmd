use crate::confirm_cmd::{confirm_high_risk, effective_workflow_risk, read_action};
use crate::execute_cmd::{
    run_command_capture_controlled, with_cwd_capture, CommandOutput, CommandTermination,
};
use crate::plan_cmd::{
    WorkflowConditionResult, WorkflowFailurePolicy, WorkflowPlan, WorkflowRisk, WorkflowStep,
    WorkflowStepKind,
};
use crate::utils::{localized, AbortSignal, Shell};

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

impl StepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Cancelled => "cancelled",
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStatus {
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowRecord {
    pub request: String,
    pub plan: WorkflowPlan,
    pub results: Vec<StepResult>,
    pub repair_attempts: u8,
    pub status: WorkflowStatus,
}

impl WorkflowRecord {
    pub fn from_partial(
        request: String,
        plan: WorkflowPlan,
        partial_results: Vec<StepResult>,
        repair_attempts: u8,
        status: WorkflowStatus,
        pending_termination: &str,
    ) -> Self {
        let results = plan
            .steps
            .iter()
            .map(|step| {
                if let Some(result) = partial_results
                    .iter()
                    .find(|result| result.step_id == step.id)
                {
                    return result.clone();
                }
                let skipped = step.run_if.as_ref().is_some_and(|condition| {
                    partial_results
                        .iter()
                        .find(|result| result.step_id == condition.step)
                        .is_some_and(|result| {
                            matches!(result.status, StepStatus::Passed | StepStatus::Failed)
                                && !matches!(
                                    (result.status, condition.result),
                                    (StepStatus::Passed, WorkflowConditionResult::Passed)
                                        | (StepStatus::Failed, WorkflowConditionResult::Failed)
                                )
                        })
                });
                StepResult {
                    step_id: step.id.clone(),
                    status: if skipped {
                        StepStatus::Skipped
                    } else {
                        StepStatus::Pending
                    },
                    exit_code: if skipped { 0 } else { -1 },
                    termination: if skipped {
                        "not_run".to_string()
                    } else {
                        pending_termination.to_string()
                    },
                    stdout: String::new(),
                    stderr: String::new(),
                }
            })
            .collect();
        Self {
            request,
            plan,
            results,
            repair_attempts,
            status,
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
        if self.is_stopped()
            || self.plan.steps.iter().any(|step| {
                step.kind == WorkflowStepKind::Check
                    && self.status_of(&step.id) == StepStatus::Pending
            })
        {
            return None;
        }
        self.plan
            .steps
            .iter()
            .filter(|step| self.status_of(&step.id) == StepStatus::Pending)
            .filter(|step| self.condition_matches(step))
            .map(|step| effective_workflow_risk(&step.command, step.risk))
            .filter(|risk| *risk > WorkflowRisk::ReadOnly)
            .max()
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
        self.block_later_modifications_after_continue_failure();
        self.apply_conditions();
        Ok(())
    }

    pub fn is_stopped(&self) -> bool {
        self.results.iter().any(|result| {
            let step = self.step(&result.step_id);
            result.status == StepStatus::Cancelled
                || (result.status == StepStatus::Failed
                    && matches!(
                        step.on_failure,
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

    fn condition_matches(&self, step: &WorkflowStep) -> bool {
        let Some(condition) = &step.run_if else {
            return true;
        };
        matches!(
            (self.status_of(&condition.step), condition.result),
            (StepStatus::Passed, WorkflowConditionResult::Passed)
                | (StepStatus::Failed, WorkflowConditionResult::Failed)
        )
    }

    fn block_later_modifications_after_continue_failure(&mut self) {
        let Some(result) = self.results.last() else {
            return;
        };
        let failed_step = self.step(&result.step_id);
        if result.status != StepStatus::Failed
            || failed_step.on_failure != WorkflowFailurePolicy::Continue
            || effective_workflow_risk(&failed_step.command, failed_step.risk)
                == WorkflowRisk::ReadOnly
        {
            return;
        }
        let failed_id = result.step_id.clone();
        let mut after_failure = false;
        for step in &self.plan.steps {
            if step.id == failed_id {
                after_failure = true;
                continue;
            }
            if after_failure
                && self.statuses[&step.id] == StepStatus::Pending
                && effective_workflow_risk(&step.command, step.risk) > WorkflowRisk::ReadOnly
            {
                self.statuses.insert(step.id.clone(), StepStatus::Skipped);
            }
        }
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
    if !prepared.needs_confirmation() {
        return Ok(true);
    }
    let risk = prepared
        .confirmation_risk()
        .expect("confirmation is required only for a risky pending step");

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

pub async fn execute_prepared_workflow(
    shell: &Shell,
    workflow: &mut PreparedWorkflow,
    abort_signal: AbortSignal,
) -> Result<WorkflowStatus> {
    while let Some(step) = workflow.next_step().cloned() {
        if step.kind == WorkflowStepKind::Check {
            anyhow::bail!("workflow check '{}' was not executed", step.id);
        }
        let command = with_cwd_capture(shell, &step.command);
        let cwd = std::env::current_dir().ok();
        let before = if effective_workflow_risk(&step.command, step.risk) > WorkflowRisk::ReadOnly {
            cwd.as_deref()
                .and_then(crate::change_report_cmd::GitSnapshot::capture)
        } else {
            None
        };
        let output = run_command_capture_controlled(shell, &command, abort_signal.clone()).await?;
        if let (Some(before), Some(cwd)) = (before, cwd) {
            if let Some(after) = crate::change_report_cmd::GitSnapshot::capture(&cwd) {
                let changes = before.changes_since(&after);
                if !changes.is_empty() {
                    println!(
                        "\n{}",
                        crate::change_report_cmd::format_recovery_report(&changes)
                    );
                }
            }
        }
        workflow.record(step_result_from_output(&step.id, output))?;
    }

    Ok(
        if workflow
            .results
            .iter()
            .any(|result| result.status == StepStatus::Cancelled)
        {
            WorkflowStatus::Cancelled
        } else if workflow.completed() {
            WorkflowStatus::Completed
        } else {
            WorkflowStatus::Failed
        },
    )
}

pub fn step_result_from_output(step_id: &str, output: CommandOutput) -> StepResult {
    if output.termination == CommandTermination::Cancelled {
        StepResult {
            step_id: step_id.to_string(),
            status: StepStatus::Cancelled,
            exit_code: output.code,
            termination: output.termination.as_str().to_string(),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    } else {
        StepResult::exited(step_id, output.code, &output.stdout, &output.stderr)
    }
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
    use crate::utils::{create_abort_signal, Shell};
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

    fn multi_modification_plan() -> WorkflowPlan {
        parse_execution_plan(
            r#"{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Apply multiple changes",
              "steps":[
                {"id":"check","kind":"check","command":"true","risk":"read_only","on_failure":"continue"},
                {"id":"write","kind":"action","command":"touch /tmp/aicmd-safe","risk":"changes_files","on_failure":"continue"},
                {"id":"remove","kind":"action","command":"rm -rf /tmp/aicmd-danger","risk":"read_only","on_failure":"stop"},
                {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"repair"}
              ]
            }"#,
        )
        .unwrap()
        .workflow()
        .unwrap()
    }

    fn conditional_destructive_plan() -> WorkflowPlan {
        parse_execution_plan(
            r#"{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Conditionally remove files",
              "steps":[
                {"id":"check","kind":"check","command":"true","risk":"read_only","on_failure":"continue"},
                {"id":"write","kind":"action","command":"touch /tmp/aicmd-safe","risk":"changes_files","on_failure":"continue"},
                {"id":"remove","kind":"action","command":"rm -rf /tmp/aicmd-danger","risk":"destructive","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
                {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"repair"}
              ]
            }"#,
        )
        .unwrap()
        .workflow()
        .unwrap()
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    fn temp_file_plan(file: &std::path::Path) -> WorkflowPlan {
        let file = shell_quote(&file.display().to_string());
        parse_execution_plan(&format!(
            r#"{{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Write and verify a temporary file",
              "steps":[
                {{"id":"write","kind":"action","command":"printf 'ok\\n' > {file}","risk":"changes_files","on_failure":"stop"}},
                {{"id":"verify","kind":"verify","command":"test \"$(cat {file})\" = ok","risk":"read_only","on_failure":"repair"}}
              ]
            }}"#
        ))
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
    fn confirmation_waits_for_checks_and_respects_resolved_conditions() {
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
    fn confirmation_aggregates_maximum_risk_across_all_pending_modifications() {
        let prepared = prepare_workflow(
            multi_modification_plan(),
            &[StepResult::exited("check", 0, "", "")],
        )
        .unwrap();

        assert_eq!(
            prepared.confirmation_risk(),
            Some(WorkflowRisk::Destructive)
        );
    }

    #[test]
    fn confirmation_excludes_condition_skipped_destructive_steps() {
        let prepared = prepare_workflow(
            conditional_destructive_plan(),
            &[StepResult::exited("check", 0, "", "")],
        )
        .unwrap();

        assert_eq!(prepared.status_of("remove"), StepStatus::Skipped);
        assert_eq!(
            prepared.confirmation_risk(),
            Some(WorkflowRisk::ChangesFiles)
        );
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
    fn modifying_failure_with_continue_stops_later_modifications() {
        let mut workflow = prepare_workflow(
            multi_modification_plan(),
            &[StepResult::exited("check", 0, "", "")],
        )
        .unwrap();
        workflow
            .record(StepResult::exited("write", 1, "", "failed"))
            .unwrap();

        assert!(!workflow.is_stopped());
        assert_eq!(workflow.status_of("remove"), StepStatus::Skipped);
        assert_eq!(workflow.next_step().unwrap().id, "verify");
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

    #[tokio::test]
    async fn temporary_file_workflow_runs_action_then_verify() {
        let root = std::env::temp_dir().join(format!("aicmd-workflow-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("result.txt");
        let plan = temp_file_plan(&file);
        let mut workflow = prepare_workflow(plan, &[]).unwrap();
        assert!(workflow.needs_confirmation());
        let shell = Shell::new("sh", "/bin/sh", "-c");
        let status = execute_prepared_workflow(&shell, &mut workflow, create_abort_signal())
            .await
            .unwrap();
        assert_eq!(status, WorkflowStatus::Completed);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ok\n");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn failed_continue_modification_does_not_run_later_modification() {
        let root = std::env::temp_dir().join(format!("aicmd-workflow-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let marker = root.join("must-not-exist.txt");
        let marker = shell_quote(&marker.display().to_string());
        let plan = parse_execution_plan(&format!(
            r#"{{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Stop later modifications",
              "steps":[
                {{"id":"fail","kind":"action","command":"false","risk":"changes_files","on_failure":"continue"}},
                {{"id":"write","kind":"action","command":"touch {marker}","risk":"changes_files","on_failure":"stop"}},
                {{"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"repair"}}
              ]
            }}"#
        ))
        .unwrap()
        .workflow()
        .unwrap();
        let mut workflow = prepare_workflow(plan, &[]).unwrap();
        assert!(workflow.needs_confirmation());
        let shell = Shell::new("sh", "/bin/sh", "-c");

        let status = execute_prepared_workflow(&shell, &mut workflow, create_abort_signal())
            .await
            .unwrap();

        assert_eq!(status, WorkflowStatus::Failed);
        assert_eq!(workflow.status_of("write"), StepStatus::Skipped);
        assert_eq!(workflow.status_of("verify"), StepStatus::Passed);
        assert!(!root.join("must-not-exist.txt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
