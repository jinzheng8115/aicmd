use crate::{config::Config, doctor_cmd, model_cmd};

use anyhow::{bail, Context, Result};
use std::{env, fs, path::PathBuf};

pub fn run_config_command(args: &[String]) -> Result<i32> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "help" | "-h" | "--help" => {
            print_usage();
            Ok(0)
        }
        "path" | "dir" | "show" | "edit" => model_cmd::run_model_command(args),
        "init" => {
            let mut model_args = vec!["init".to_string(), "--from-env".to_string()];
            for arg in &args[1..] {
                match arg.as_str() {
                    "--force" => model_args.push(arg.clone()),
                    _ => bail!("Unknown option for config init: {arg}"),
                }
            }
            model_cmd::run_model_command(&model_args)
        }
        "mcp" => {
            println!("{}", mcp_config_path().display());
            Ok(0)
        }
        "summary" | "ai-summary" => run_summary_command(&args[1..]),
        "doctor" => doctor_cmd::run_doctor_command(),
        _ => {
            eprintln!("Unknown config command: {cmd}");
            print_usage();
            Ok(2)
        }
    }
}

fn run_summary_command(args: &[String]) -> Result<i32> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    match action {
        "status" => {
            let path = Config::config_file();
            let content = read_config_file(&path)?;
            let enabled = read_ai_summary_setting(&content).unwrap_or(true);
            println!(
                "ai_summary: {} / AI summary 默认{}",
                enabled,
                if enabled { "开启" } else { "关闭" }
            );
            println!("config: {}", path.display());
            Ok(0)
        }
        "on" | "enable" | "enabled" | "true" => set_ai_summary(true),
        "off" | "disable" | "disabled" | "false" => set_ai_summary(false),
        "help" | "-h" | "--help" => {
            print_summary_usage();
            Ok(0)
        }
        _ => bail!("Unknown summary action: {action}. Use: on, off, or status"),
    }
}

fn set_ai_summary(enabled: bool) -> Result<i32> {
    let path = Config::config_file();
    let content = read_config_file(&path)?;
    let updated = set_ai_summary_in_config(&content, enabled);
    fs::write(&path, updated)
        .with_context(|| format!("failed to write config: {}", path.display()))?;
    println!(
        "ai_summary set to {} / AI summary 已{}",
        enabled,
        if enabled { "开启" } else { "关闭" }
    );
    println!("config: {}", path.display());
    Ok(0)
}

fn read_config_file(path: &PathBuf) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read config: {}", path.display()))
}

fn read_ai_summary_setting(content: &str) -> Option<bool> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        let value = trimmed.strip_prefix("ai_summary:")?.trim();
        match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    })
}

fn set_ai_summary_in_config(content: &str, enabled: bool) -> String {
    let value = if enabled { "true" } else { "false" };
    let mut found = false;
    let mut lines = content
        .lines()
        .map(|line| {
            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];
            if line.trim_start().starts_with("ai_summary:") {
                found = true;
                format!("{indent}ai_summary: {value}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();
    if !found {
        let insert_at = lines
            .iter()
            .position(|line| line.trim_start().starts_with("save:"))
            .unwrap_or(lines.len());
        lines.insert(insert_at, format!("ai_summary: {value}"));
    }
    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn mcp_config_path() -> PathBuf {
    env::var("AICMD_MCP_CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Config::config_dir().join("mcp.json"))
}

fn print_usage() {
    println!(
        r#"Usage: aicmd config <command>

Manage common AICmd configuration tasks.

用法：aicmd config <命令>

管理常用 AICmd 配置任务。

Commands / 命令:
  init [--force]     Generate config.yaml from .env / 从 .env 生成 config.yaml
  path               Print config.yaml path / 输出 config.yaml 路径
  dir                Print config directory / 输出配置目录
  show               Print config.yaml / 输出 config.yaml
  edit               Open config.yaml in $EDITOR / 用 $EDITOR 编辑 config.yaml
  summary [status]   Show AI summary default / 查看 AI summary 默认状态
  summary on         Enable AI summary by default / 默认开启 AI summary
  summary off        Disable AI summary by default / 默认关闭 AI summary
  mcp                Print mcp.json path / 输出 mcp.json 路径
  doctor             Run aicmd doctor / 运行 aicmd doctor
  help               Show this help / 显示帮助"#
    );
}

fn print_summary_usage() {
    println!(
        r#"Usage: aicmd config summary <status|on|off>

Manage the default AI summary setting in config.yaml.

用法：aicmd config summary <status|on|off>

管理 config.yaml 中的 AI summary 默认开关。"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_ai_summary_replaces_existing_value() {
        let input = "model: test\nai_summary: true\nsave: true\n";
        let output = set_ai_summary_in_config(input, false);
        assert!(output.contains("\nai_summary: false\n"));
        assert!(!output.contains("\nai_summary: true\n"));
    }

    #[test]
    fn set_ai_summary_inserts_before_save_when_missing() {
        let input = "model: test\nstream: false\nsave: true\n";
        let output = set_ai_summary_in_config(input, false);
        assert_eq!(
            output,
            "model: test\nstream: false\nai_summary: false\nsave: true\n"
        );
    }

    #[test]
    fn read_ai_summary_setting_reads_boolean() {
        assert_eq!(read_ai_summary_setting("ai_summary: false\n"), Some(false));
        assert_eq!(read_ai_summary_setting("ai_summary: true\n"), Some(true));
        assert_eq!(read_ai_summary_setting("model: test\n"), None);
    }
}
