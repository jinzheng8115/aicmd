use crate::{
    config::{ensure_parent_exists, Config},
    preflight_cmd::PreflightCheck,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

const CACHE_FILE_NAME: &str = "command-cache.yaml";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandCacheRecord {
    pub key: String,
    pub task: String,
    pub shell: String,
    pub os: String,
    pub command: String,
    #[serde(default)]
    pub preflight: Vec<PreflightCheck>,
    #[serde(default)]
    pub has_preflight: bool,
    pub success_count: u32,
    pub last_used_at: String,
}

pub fn lookup(task: &str, shell: &str, os: &str) -> Option<CommandCacheRecord> {
    if is_sensitive_task(task) {
        return None;
    }
    let key = cache_key(task, shell, os);
    read_records()
        .ok()?
        .into_iter()
        .find(|record| record.key == key && record.has_preflight)
}

pub fn record_success(
    task: &str,
    shell: &str,
    os: &str,
    command: &str,
    preflight: &[PreflightCheck],
) -> Result<()> {
    if is_sensitive_task(task) || command.trim().is_empty() {
        return Ok(());
    }
    let key = cache_key(task, shell, os);
    let mut records = read_records().unwrap_or_default();
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(record) = records.iter_mut().find(|record| record.key == key) {
        record.command = command.to_string();
        record.preflight = preflight.to_vec();
        record.has_preflight = true;
        record.success_count = record.success_count.saturating_add(1);
        record.last_used_at = now;
    } else {
        records.push(CommandCacheRecord {
            key,
            task: task.to_string(),
            shell: shell.to_string(),
            os: os.to_string(),
            command: command.to_string(),
            preflight: preflight.to_vec(),
            has_preflight: true,
            success_count: 1,
            last_used_at: now,
        });
    }
    write_records(&records)
}

fn cache_path() -> PathBuf {
    Config::local_path(CACHE_FILE_NAME)
}

fn read_records() -> Result<Vec<CommandCacheRecord>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read command cache: {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(vec![]);
    }
    serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse command cache: {}", path.display()))
}

fn write_records(records: &[CommandCacheRecord]) -> Result<()> {
    let path = cache_path();
    ensure_parent_exists(&path)?;
    let content = serde_yaml::to_string(records).context("failed to serialize command cache")?;
    fs::write(&path, content)
        .with_context(|| format!("failed to write command cache: {}", path.display()))
}

fn cache_key(task: &str, shell: &str, os: &str) -> String {
    let normalized = normalize_task(task);
    let value = format!("{os}\n{shell}\n{normalized}");
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn normalize_task(task: &str) -> String {
    task.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_sensitive_task(task: &str) -> bool {
    let lower = task.to_lowercase();
    ["password", "token", "secret", "api key", "密钥", "密码"]
        .iter()
        .any(|term| lower.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_task_collapses_whitespace_and_lowercases_ascii() {
        assert_eq!(normalize_task("  List   FILES  目录  "), "list files 目录");
    }

    #[test]
    fn sensitive_task_detection_catches_secret_terms() {
        assert!(is_sensitive_task("show api key"));
        assert!(is_sensitive_task("查看密码"));
        assert!(!is_sensitive_task("当前目录有多少文件"));
    }

    #[test]
    fn cache_key_includes_shell_and_os() {
        let zsh_key = cache_key("当前目录有多少文件", "zsh", "macos");
        let bash_key = cache_key("当前目录有多少文件", "bash", "macos");
        assert_ne!(zsh_key, bash_key);
    }

    #[test]
    fn legacy_cache_record_without_preflight_is_not_reusable() {
        let yaml = r#"
- key: old
  task: pwd
  shell: zsh
  os: macos
  command: pwd
  success_count: 1
  last_used_at: now
"#;
        let records: Vec<CommandCacheRecord> = serde_yaml::from_str(yaml).unwrap();
        assert!(!records[0].has_preflight);
        assert!(records[0].preflight.is_empty());
    }
}
