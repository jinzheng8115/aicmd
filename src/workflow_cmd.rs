use crate::confirm_cmd::{confirm_high_risk, effective_workflow_risk, read_action};
use crate::plan_cmd::{
    WorkflowConditionResult, WorkflowFailurePolicy, WorkflowPlan, WorkflowRisk, WorkflowStep,
    WorkflowStepKind,
};

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
        self.next_step().is_some_and(|step| {
            effective_workflow_risk(&step.command, step.risk) > WorkflowRisk::ReadOnly
        })
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
            "Workflow confirmation / 工作流确认: {}",
            prepared.plan.summary
        ),
        "Prepared plan / 已准备计划:".to_string(),
    ];

    for step in &prepared.plan.steps {
        let status = prepared.status_of(&step.id);
        match status {
            StepStatus::Pending => lines.push(format!(
                "  [pending / 待执行] {} ({}) : {}",
                step.id,
                workflow_risk_label(effective_workflow_risk(&step.command, step.risk)),
                step.command
            )),
            StepStatus::Skipped => lines.push(format!("  [skipped / 已跳过] {}", step.id)),
            StepStatus::Passed => lines.push(format!("  [passed / 已完成] {}", step.id)),
            StepStatus::Failed => lines.push(format!("  [failed / 已失败] {}", step.id)),
            StepStatus::Cancelled => lines.push(format!("  [cancelled / 已取消] {}", step.id)),
        }
    }
    lines.join("\n")
}

pub fn confirm_workflow(prepared: &PreparedWorkflow) -> Result<bool> {
    let risks: Vec<_> = prepared
        .plan
        .steps
        .iter()
        .filter(|step| prepared.status_of(&step.id) == StepStatus::Pending)
        .map(|step| effective_workflow_risk(&step.command, step.risk))
        .collect();
    if !risks.iter().any(|risk| *risk > WorkflowRisk::ReadOnly) {
        return Ok(true);
    }

    println!("{}", render_workflow_confirmation(prepared));
    if read_action(&['y', 'n'], 'n', "Execute this plan? / 执行此计划？ [y/N] ")? != 'y' {
        return Ok(false);
    }
    if risks.contains(&WorkflowRisk::Destructive)
        && !confirm_high_risk(
            "Destructive step detected. Continue? / 检测到破坏性步骤，确认继续？",
        )?
    {
        return Ok(false);
    }
    Ok(true)
}

fn workflow_risk_label(risk: WorkflowRisk) -> &'static str {
    match risk {
        WorkflowRisk::ReadOnly => "read-only / 只读",
        WorkflowRisk::ChangesFiles => "changes files / 会修改文件",
        WorkflowRisk::ChangesSystem => "changes system / 会修改系统或文件",
        WorkflowRisk::Destructive => "destructive / 可能造成破坏",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_cmd::{parse_execution_plan, WorkflowPlan};

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
        let text = render_workflow_confirmation(&prepared);
        assert!(text.contains("verify"));
        assert!(text.contains("skipped"));
        assert!(!text.contains("brew install tool"));
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
