use crate::config::GlobalConfig;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn eval_tool_calls(_config: &GlobalConfig, calls: Vec<ToolCall>) -> Result<Vec<ToolResult>> {
    if calls.is_empty() {
        return Ok(vec![]);
    }
    bail!("Tool calls are disabled in the focused AICmd command workflow")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolResult {
    pub call: ToolCall,
    pub output: Value,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
    pub id: Option<String>,
}

impl ToolCall {
    pub fn new(name: String, arguments: Value, id: Option<String>) -> Self {
        Self {
            name,
            arguments,
            id,
        }
    }
}
