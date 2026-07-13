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
        self.plan.steps.iter().any(|step| {
            self.status_of(&step.id) == StepStatus::Pending && step.risk > WorkflowRisk::ReadOnly
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
            result.status == StepStatus::Failed
                && matches!(
                    self.step(&result.step_id).on_failure,
                    WorkflowFailurePolicy::Stop | WorkflowFailurePolicy::Repair
                )
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
    fn failed_check_enables_conditioned_install() {
        let plan = fixture_plan();
        let results = vec![StepResult::exited("check", 1, "", "not found")];
        let prepared = prepare_workflow(plan, &results).unwrap();
        assert_eq!(prepared.status_of("install"), StepStatus::Pending);
        assert!(prepared.needs_confirmation());
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
