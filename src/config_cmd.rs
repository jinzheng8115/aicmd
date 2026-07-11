use crate::{config::Config, doctor_cmd, model_cmd, utils::localized};

use anyhow::{bail, Context, Result};
use serde_yaml::Value;
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
        "status" => run_status_command(),
        "summary" | "ai-summary" => run_summary_command(&args[1..]),
        "language" | "lang" => run_language_command(&args[1..]),
        "doctor" => doctor_cmd::run_doctor_command(),
        _ => {
            eprintln!("Unknown config command: {cmd}");
            print_usage();
            Ok(2)
        }
    }
}

fn run_status_command() -> Result<i32> {
    let config_path = Config::config_file();
    let config_content = read_config_file(&config_path)?;
    let config_yaml: Value = serde_yaml::from_str(&config_content)
        .with_context(|| format!("failed to parse config: {}", config_path.display()))?;
    let model = yaml_string(&config_yaml, "model").unwrap_or_else(|| "unknown".to_string());
    let language = yaml_string(&config_yaml, "language").unwrap_or_else(|| "zh".to_string());
    let temperature = yaml_scalar_display(&config_yaml, "temperature")
        .unwrap_or_else(|| localized("模型服务默认值", "provider default").to_string());
    let ai_summary = yaml_bool(&config_yaml, "ai_summary").unwrap_or(false);
    let mcp_path = mcp_config_path();
    let mcp_status = if mcp_path.exists() {
        localized("已配置", "configured")
    } else {
        localized("未配置", "missing")
    };
    let search_status = if mcp_path.exists() && mcp_config_has_search_command(&mcp_path) {
        localized("已配置", "configured")
    } else {
        localized("未配置", "missing")
    };
    let session = current_default_session_name();

    println!("{}", localized("AICmd 配置状态", "AICmd config status"));
    println!(
        "{}: {}",
        localized("配置文件", "Config file"),
        config_path.display()
    );
    println!("{}: {model}", localized("默认模型", "Default model"));
    println!("{}: {language}", localized("语言", "Language"));
    println!("{}: {temperature}", localized("温度", "Temperature"));
    println!(
        "{}: {}",
        localized("AI 总结", "AI summary"),
        if ai_summary {
            localized("开启", "on")
        } else {
            localized("关闭", "off")
        }
    );
    println!("{}: {mcp_status}", localized("MCP 配置", "MCP config"));
    println!("{}: {search_status}", localized("搜索", "Search"));
    println!("{}: {session}", localized("会话", "Session"));
    Ok(0)
}

fn run_language_command(args: &[String]) -> Result<i32> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    let path = Config::config_file();
    let content = read_config_file(&path)?;
    match action {
        "status" => {
            let language = read_language_setting(&content).unwrap_or("zh");
            println!("language: {language}");
        }
        "zh" | "chinese" => {
            fs::write(&path, set_language_in_config(&content, "zh"))?;
            println!("语言已设置为中文");
        }
        "en" | "english" => {
            fs::write(&path, set_language_in_config(&content, "en"))?;
            println!("Language set to English");
        }
        _ => bail!("Use: aicmd config language <zh|en|status>"),
    }
    Ok(0)
}

fn read_language_setting(content: &str) -> Option<&str> {
    content.lines().find_map(|line| {
        line.trim()
            .strip_prefix("language:")
            .map(str::trim)
            .filter(|value| matches!(*value, "zh" | "en"))
    })
}

fn set_language_in_config(content: &str, language: &str) -> String {
    let mut found = false;
    let mut lines = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("language:") {
                found = true;
                format!("language: {language}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();
    if !found {
        let insert_at = lines
            .iter()
            .position(|line| line.trim_start().starts_with("temperature:"))
            .unwrap_or(1.min(lines.len()));
        lines.insert(insert_at, format!("language: {language}"));
    }
    let mut output = lines.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn yaml_string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_string)
}

fn yaml_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key)?.as_bool()
}

fn yaml_scalar_display(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?;
    if value.is_null() {
        return None;
    }
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_f64() {
        return Some(number.to_string());
    }
    if let Some(enabled) = value.as_bool() {
        return Some(enabled.to_string());
    }
    None
}

fn mcp_config_has_search_command(path: &PathBuf) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    let root = value.get("mcp").unwrap_or(&value);
    root.get("commands")
        .and_then(|v| v.as_object())
        .is_some_and(|commands| commands.contains_key("search"))
}

fn current_default_session_name() -> String {
    let beijing = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid timezone"));
    format!("cmd-{}", beijing.format("%Y%m%d"))
}

fn run_summary_command(args: &[String]) -> Result<i32> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    match action {
        "status" => {
            let path = Config::config_file();
            let content = read_config_file(&path)?;
            let enabled = read_ai_summary_setting(&content).unwrap_or(false);
            println!(
                "ai_summary: {} ({})",
                enabled,
                if enabled {
                    localized("默认开启", "enabled by default")
                } else {
                    localized("默认关闭", "disabled by default")
                }
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
        "{}: {}",
        localized("AI summary 已更新", "AI summary updated"),
        enabled
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
  status             Show active safe settings / 查看当前安全配置状态
  summary [status]   Show AI summary default / 查看 AI summary 默认状态
  summary on         Enable AI summary by default / 默认开启 AI summary
  summary off        Disable AI summary by default / 默认关闭 AI summary
  language zh|en     Select terminal language / 选择终端显示语言
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

    #[test]
    fn language_setting_defaults_and_updates_cleanly() {
        let input = "model: test\ntemperature: 0\n";
        assert_eq!(read_language_setting(input), None);
        let output = set_language_in_config(input, "en");
        assert_eq!(read_language_setting(&output), Some("en"));
    }
}
