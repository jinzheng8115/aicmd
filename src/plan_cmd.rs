use crate::{
    client::call_chat_completions_raw_controlled,
    config::{GlobalConfig, Input, SHELL_ROLE},
    preflight_cmd::{validate_checks, PreflightCheck},
    utils::{AbortSignal, ProgressStage, RetryBudget},
};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanMode {
    Direct,
    Script,
    Search,
    Diagnose,
    Workflow,
}

impl fmt::Display for PlanMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Direct => "direct",
            Self::Script => "script",
            Self::Search => "search",
            Self::Diagnose => "diagnose",
            Self::Workflow => "workflow",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind {
    Check,
    Action,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRisk {
    ReadOnly,
    ChangesFiles,
    ChangesSystem,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFailurePolicy {
    Continue,
    Stop,
    Repair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConditionResult {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCondition {
    pub step: String,
    pub result: WorkflowConditionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStep {
    pub id: String,
    pub kind: WorkflowStepKind,
    pub command: String,
    pub risk: WorkflowRisk,
    #[serde(default)]
    pub run_if: Option<WorkflowCondition>,
    pub on_failure: WorkflowFailurePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPlan {
    pub summary: String,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub mode: PlanMode,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub problem: String,
    pub preflight: Vec<PreflightCheck>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
}

impl ExecutionPlan {
    pub fn workflow(&self) -> Option<WorkflowPlan> {
        (self.mode == PlanMode::Workflow).then(|| WorkflowPlan {
            summary: self.summary.clone(),
            steps: self.steps.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCommand {
    pub command: String,
    pub preflight: Vec<PreflightCheck>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Command,
    Search,
    Diagnose,
    Workflow,
}

pub fn route_kind(mode: &PlanMode) -> RouteKind {
    match mode {
        PlanMode::Direct | PlanMode::Script => RouteKind::Command,
        PlanMode::Search => RouteKind::Search,
        PlanMode::Diagnose => RouteKind::Diagnose,
        PlanMode::Workflow => RouteKind::Workflow,
    }
}

pub fn parse_execution_plan(raw: &str) -> Result<ExecutionPlan> {
    let plan: ExecutionPlan = serde_json::from_str(raw)?;
    let has_command = !plan.command.trim().is_empty();
    let has_query = !plan.query.trim().is_empty();
    let has_problem = !plan.problem.trim().is_empty();
    validate_checks(&plan.preflight)?;

    if plan.mode != PlanMode::Workflow && (!plan.summary.is_empty() || !plan.steps.is_empty()) {
        bail!("Invalid execution plan: only 'workflow' can include summary or steps")
    }

    match plan.mode {
        PlanMode::Direct | PlanMode::Script if !has_command => {
            bail!(
                "Invalid execution plan: '{}' requires a non-empty command",
                plan.mode
            )
        }
        PlanMode::Search if !has_query => {
            bail!("Invalid execution plan: 'search' requires a non-empty query")
        }
        PlanMode::Diagnose if !has_problem => {
            bail!("Invalid execution plan: 'diagnose' requires a non-empty problem")
        }
        PlanMode::Direct | PlanMode::Script if has_query || has_problem => {
            bail!(
                "Invalid execution plan: '{}' cannot include query or problem",
                plan.mode
            )
        }
        PlanMode::Search if has_command || has_problem || !plan.preflight.is_empty() => {
            bail!("Invalid execution plan: 'search' cannot include command or problem")
        }
        PlanMode::Diagnose if has_command || has_query || !plan.preflight.is_empty() => {
            bail!("Invalid execution plan: 'diagnose' cannot include command or query")
        }
        PlanMode::Workflow
            if has_command || has_query || has_problem || !plan.preflight.is_empty() =>
        {
            bail!("Invalid execution plan: 'workflow' cannot include command, query, problem, or preflight")
        }
        PlanMode::Workflow => {
            validate_workflow(&plan)?;
            Ok(plan)
        }
        _ => Ok(plan),
    }
}

fn validate_workflow(plan: &ExecutionPlan) -> Result<()> {
    if plan.summary.trim().is_empty() {
        bail!("Invalid workflow plan: summary must not be empty")
    }
    if plan.steps.is_empty() {
        bail!("Invalid workflow plan: steps must not be empty")
    }

    let mut ids: HashSet<String> = HashSet::new();
    let mut checks: HashSet<String> = HashSet::new();
    let mut has_verify = false;

    for step in &plan.steps {
        if step.id.trim().is_empty() {
            bail!("Invalid workflow plan: step id must not be empty")
        }
        if step.command.trim().is_empty() {
            bail!("Invalid workflow plan: step command must not be empty")
        }
        if !ids.insert(step.id.clone()) {
            bail!("Invalid workflow plan: duplicate step id '{}'", step.id)
        }
        if let Some(condition) = &step.run_if {
            if !checks.contains(&condition.step) {
                bail!(
                    "Invalid workflow plan: run_if must reference an earlier check '{}'",
                    condition.step
                )
            }
        }

        match step.kind {
            WorkflowStepKind::Check => {
                if step.risk != WorkflowRisk::ReadOnly {
                    bail!("Invalid workflow plan: check steps must be read_only")
                }
                checks.insert(step.id.clone());
            }
            WorkflowStepKind::Action => {}
            WorkflowStepKind::Verify => {
                if step.risk != WorkflowRisk::ReadOnly {
                    bail!("Invalid workflow plan: verify steps must be read_only")
                }
                has_verify = true;
            }
        }
    }

    if !has_verify {
        bail!("Invalid workflow plan: requires at least one verify step")
    }
    Ok(())
}

pub fn render_execution_plan(plan: &ExecutionPlan) -> Result<String> {
    Ok(serde_json::to_string_pretty(plan)?)
}

pub fn parse_generated_command(raw: &str) -> Result<GeneratedCommand> {
    let generated: GeneratedCommand = serde_json::from_str(raw)?;
    if generated.command.trim().is_empty() {
        bail!("generated command must not be empty");
    }
    validate_checks(&generated.preflight)?;
    Ok(generated)
}

fn parse_planner_response(raw: &str) -> Result<ExecutionPlan> {
    parse_execution_plan(raw)
}

pub async fn request_execution_plan(
    config: &GlobalConfig,
    input: &Input,
    abort_signal: AbortSignal,
    retry_budget: &RetryBudget,
) -> Result<ExecutionPlan> {
    let role = config.read().retrieve_role(SHELL_ROLE)?;
    let planner_input = input.clone().with_role(role);
    let client = planner_input.create_client()?;
    config.write().before_chat_completion(&planner_input)?;
    let (raw, _) = call_chat_completions_raw_controlled(
        &planner_input,
        client.as_ref(),
        abort_signal,
        retry_budget,
        ProgressStage::new("正在生成执行计划", "Generating execution plan"),
    )
    .await?;
    parse_planner_response(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects_invalid_execution_plans() -> anyhow::Result<()> {
        assert_eq!(
            parse_execution_plan(
                r#"{"mode":"direct","command":"pwd","query":"","problem":"","preflight":[]}"#,
            )?
            .mode,
            PlanMode::Direct,
        );
        assert!(
            parse_execution_plan(
                "```json\n{\"mode\":\"direct\",\"command\":\"pwd\",\"query\":\"\",\"problem\":\"\",\"preflight\":[]}\n```"
            )
            .is_err()
        );
        assert!(parse_execution_plan(
            r#"{"mode":"direct","command":"","query":"x","problem":"","preflight":[]}"#
        )
        .is_err());
        assert!(parse_execution_plan(
            r#"{"mode":"search","command":"","query":"rust","problem":"","preflight":[],"extra":true}"#
        )
        .is_err());
        assert!(parse_execution_plan(
            r#"{"mode":"search","command":"","query":"rust","problem":"","preflight":[{"type":"command_exists","value":"git","failure_message":"f","suggestion":"s"}]}"#
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn planner_response_rejects_surrounding_or_fenced_json() {
        assert!(parse_planner_response(
            "Here is the plan: {\"mode\":\"direct\",\"command\":\"pwd\",\"query\":\"\",\"problem\":\"\",\"preflight\":[]}"
        )
        .is_err());
        assert!(
            parse_planner_response(
                "```json\n{\"mode\":\"direct\",\"command\":\"pwd\",\"query\":\"\",\"problem\":\"\",\"preflight\":[]}\n```"
            )
            .is_err()
        );
    }

    #[test]
    fn maps_plan_modes_to_routes() {
        assert_eq!(route_kind(&PlanMode::Direct), RouteKind::Command);
        assert_eq!(route_kind(&PlanMode::Script), RouteKind::Command);
        assert_eq!(route_kind(&PlanMode::Search), RouteKind::Search);
        assert_eq!(route_kind(&PlanMode::Diagnose), RouteKind::Diagnose);
        assert_eq!(route_kind(&PlanMode::Workflow), RouteKind::Workflow);
    }

    #[test]
    fn renders_validated_plan_for_dry_run() -> anyhow::Result<()> {
        let plan = parse_execution_plan(
            r#"{"mode":"search","command":"","query":"Rust docs","problem":"","preflight":[]}"#,
        )?;

        let rendered = render_execution_plan(&plan)?;

        assert!(rendered.contains("\"mode\": \"search\""));
        assert!(rendered.contains("\"query\": \"Rust docs\""));
        Ok(())
    }

    #[test]
    fn parses_strict_generated_command_with_preflight() -> anyhow::Result<()> {
        let generated = parse_generated_command(
            r#"{"command":"python3 task.py","preflight":[{"type":"command_exists","value":"python3","failure_message":"未找到 Python 3","suggestion":"请先安装 Python 3"}]}"#,
        )?;
        assert_eq!(generated.command, "python3 task.py");
        assert_eq!(generated.preflight.len(), 1);
        assert!(parse_generated_command(r#"{"command":"pwd","preflight":[],"extra":1}"#).is_err());
        assert!(parse_generated_command(r#"{"command":"","preflight":[]}"#).is_err());
        Ok(())
    }

    #[test]
    fn parses_strict_workflow_plan() -> anyhow::Result<()> {
        let plan = parse_execution_plan(
            r#"{
      "mode":"workflow","command":"","query":"","problem":"","preflight":[],
      "summary":"Install tool",
      "steps":[
        {"id":"check","kind":"check","command":"command -v tool","risk":"read_only","on_failure":"continue"},
        {"id":"install","kind":"action","command":"brew install tool","risk":"changes_system","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
        {"id":"verify","kind":"verify","command":"tool --version","risk":"read_only","on_failure":"repair"}
      ]
    }"#,
        )?;
        assert_eq!(plan.mode, PlanMode::Workflow);
        assert_eq!(plan.workflow().unwrap().steps.len(), 3);
        Ok(())
    }

    #[test]
    fn rejects_invalid_workflow_relationships() {
        let duplicate = workflow_json_with_steps(
            r#"[
      {"id":"x","kind":"check","command":"true","risk":"read_only","on_failure":"continue"},
      {"id":"x","kind":"verify","command":"true","risk":"read_only","on_failure":"stop"}
    ]"#,
        );
        assert!(parse_execution_plan(&duplicate).is_err());

        let forward_reference = workflow_json_with_steps(
            r#"[
      {"id":"install","kind":"action","command":"true","risk":"changes_files","run_if":{"step":"later","result":"failed"},"on_failure":"stop"},
      {"id":"later","kind":"check","command":"false","risk":"read_only","on_failure":"continue"},
      {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"stop"}
    ]"#,
        );
        assert!(parse_execution_plan(&forward_reference).is_err());
    }

    #[test]
    fn workflow_requires_read_only_verification() {
        let no_verify = workflow_json_with_steps(
            r#"[
      {"id":"action","kind":"action","command":"touch x","risk":"changes_files","on_failure":"stop"}
    ]"#,
        );
        assert!(parse_execution_plan(&no_verify).is_err());

        let unsafe_check = workflow_json_with_steps(
            r#"[
      {"id":"check","kind":"check","command":"touch x","risk":"changes_files","on_failure":"stop"},
      {"id":"verify","kind":"verify","command":"test -f x","risk":"read_only","on_failure":"stop"}
    ]"#,
        );
        assert!(parse_execution_plan(&unsafe_check).is_err());
    }

    #[test]
    fn workflow_rejects_unknown_step_fields_and_non_workflow_steps() {
        let unknown_step_field = workflow_json_with_steps(
            r#"[
      {"id":"check","kind":"check","command":"true","risk":"read_only","on_failure":"continue","extra":true},
      {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"stop"}
    ]"#,
        );
        assert!(parse_execution_plan(&unknown_step_field).is_err());
        assert!(parse_execution_plan(
            r#"{"mode":"direct","command":"pwd","query":"","problem":"","preflight":[],"summary":"not allowed","steps":[]}"#
        )
        .is_err());
    }

    fn workflow_json_with_steps(steps: &str) -> String {
        format!(
            r#"{{"mode":"workflow","command":"","query":"","problem":"","preflight":[],"summary":"test","steps":{steps}}}"#
        )
    }
}
