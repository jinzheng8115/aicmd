use crate::{
    client::call_chat_completions,
    config::{GlobalConfig, Input, SHELL_ROLE},
    utils::AbortSignal,
};

use anyhow::{bail, Result};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub mode: PlanMode,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub problem: String,
}

pub fn parse_execution_plan(raw: &str) -> Result<ExecutionPlan> {
    let plan: ExecutionPlan = serde_json::from_str(raw)?;
    let has_command = !plan.command.trim().is_empty();
    let has_query = !plan.query.trim().is_empty();
    let has_problem = !plan.problem.trim().is_empty();

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
        PlanMode::Search if has_command || has_problem => {
            bail!("Invalid execution plan: 'search' cannot include command or problem")
        }
        PlanMode::Diagnose if has_command || has_query => {
            bail!("Invalid execution plan: 'diagnose' cannot include command or query")
        }
        _ => Ok(plan),
    }
}

pub async fn request_execution_plan(
    config: &GlobalConfig,
    input: &Input,
    abort_signal: AbortSignal,
) -> Result<ExecutionPlan> {
    let role = config.read().retrieve_role(SHELL_ROLE)?;
    let planner_input = Input::from_str(config, &input.text(), Some(role));
    let client = planner_input.create_client()?;
    config.write().before_chat_completion(&planner_input)?;
    let (raw, _) =
        call_chat_completions(&planner_input, false, false, client.as_ref(), abort_signal).await?;
    parse_execution_plan(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects_invalid_execution_plans() -> anyhow::Result<()> {
        assert_eq!(
            parse_execution_plan(r#"{"mode":"direct","command":"pwd"}"#)?.mode,
            PlanMode::Direct,
        );
        assert!(
            parse_execution_plan("```json\n{\"mode\":\"direct\",\"command\":\"pwd\"}\n```")
                .is_err()
        );
        assert!(parse_execution_plan(r#"{"mode":"direct","command":"","query":"x"}"#).is_err());
        assert!(parse_execution_plan(r#"{"mode":"search","query":"rust","extra":true}"#).is_err());
        Ok(())
    }
}
