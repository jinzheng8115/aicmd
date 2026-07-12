use crate::{
    client::call_chat_completions_raw_controlled,
    config::{GlobalConfig, Input, SHELL_ROLE},
    preflight_cmd::{validate_checks, PreflightCheck},
    utils::{AbortSignal, ProgressStage, RetryBudget},
};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanMode {
    Direct,
    Script,
    Search,
    Diagnose,
}

impl fmt::Display for PlanMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Direct => "direct",
            Self::Script => "script",
            Self::Search => "search",
            Self::Diagnose => "diagnose",
        })
    }
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
}

pub fn route_kind(mode: &PlanMode) -> RouteKind {
    match mode {
        PlanMode::Direct | PlanMode::Script => RouteKind::Command,
        PlanMode::Search => RouteKind::Search,
        PlanMode::Diagnose => RouteKind::Diagnose,
    }
}

pub fn parse_execution_plan(raw: &str) -> Result<ExecutionPlan> {
    let plan: ExecutionPlan = serde_json::from_str(raw)?;
    let has_command = !plan.command.trim().is_empty();
    let has_query = !plan.query.trim().is_empty();
    let has_problem = !plan.problem.trim().is_empty();
    validate_checks(&plan.preflight)?;

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
        _ => Ok(plan),
    }
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
}
