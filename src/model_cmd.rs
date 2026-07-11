use crate::config::Config;

use anyhow::{bail, Context, Result};
use std::{
    collections::HashMap,
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub fn run_model_command(args: &[String]) -> Result<i32> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "init" => {
            let mut force = false;
            let mut from_env = false;
            for arg in &args[1..] {
                match arg.as_str() {
                    "--force" => force = true,
                    "--from-env" => from_env = true,
                    _ => bail!("Unknown option for init: {arg}"),
                }
            }
            init_config(force, from_env)?;
        }
        "path" => println!("{}", config_path().display()),
        "dir" => println!("{}", config_dir().display()),
        "show" => {
            let path = config_path();
            if !path.exists() {
                bail!(
                    "config not found: {}\nrun 'aicmd init --from-env' to create it",
                    path.display()
                );
            }
            print!("{}", fs::read_to_string(&path)?);
        }
        "edit" => {
            let path = config_path();
            if !path.exists() {
                init_config(false, false)?;
            }
            let editor = env::var("EDITOR").unwrap_or_else(|_| {
                if cfg!(windows) {
                    "notepad".into()
                } else {
                    "vi".into()
                }
            });
            let status = Command::new(editor).arg(path).status()?;
            return Ok(status.code().unwrap_or_default());
        }
        "help" | "-h" | "--help" => print_usage(),
        _ => {
            eprintln!("Unknown command: {cmd}");
            print_usage();
            return Ok(2);
        }
    }
    Ok(0)
}

fn print_usage() {
    println!(
        r#"Usage: aicmd model <command>

Manage the AICmd runtime config at ~/.aicmd/config.yaml, including LLM settings.
The recommended setup is: copy .env.example to .env, fill the model values, then run `aicmd init --from-env`.

用法：aicmd model <命令>

管理 AICmd 运行时配置 ~/.aicmd/config.yaml，其中包含 LLM 设置。
推荐流程：复制 .env.example 为 .env，填写模型参数，然后运行 `aicmd init --from-env`。

Commands / 命令:
  init                Create runtime config.yaml / 创建运行时 config.yaml
  init --from-env     Require .env and generate config.yaml / 必须读取 .env 并生成 config.yaml
  init --force        Overwrite runtime config.yaml / 覆盖运行时配置
  path                Print runtime config.yaml path / 输出运行时 config.yaml 路径
  dir                 Print runtime config directory / 输出运行时配置目录
  show                Print runtime config.yaml / 输出运行时配置
  edit                Open runtime config.yaml in $EDITOR / 用 $EDITOR 编辑运行时配置
  help                Show this help / 显示帮助"#
    );
}

fn config_dir() -> PathBuf {
    Config::config_dir()
}

fn config_path() -> PathBuf {
    Config::config_file()
}

fn mcp_config_path() -> PathBuf {
    env::var("AICMD_MCP_CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir().join("mcp.json"))
}

fn find_env_file() -> Option<PathBuf> {
    if let Ok(path) = env::var("AICMD_MODEL_ENV") {
        return Some(PathBuf::from(path));
    }
    let cwd_env = PathBuf::from(".env");
    if cwd_env.exists() {
        return Some(cwd_env);
    }
    let config_env = Config::env_file();
    if config_env.exists() {
        return Some(config_env);
    }
    None
}

fn find_mcp_file(env_file: Option<&Path>) -> Option<PathBuf> {
    if let Ok(path) = env::var("AICMD_MCP_SOURCE") {
        return Some(PathBuf::from(path));
    }
    if let Some(env_file) = env_file {
        if let Some(parent) = env_file.parent() {
            let sibling_mcp = parent.join("mcp.json");
            if sibling_mcp.exists() {
                return Some(sibling_mcp);
            }
        }
    }
    let cwd_mcp = PathBuf::from("mcp.json");
    if cwd_mcp.exists() {
        return Some(cwd_mcp);
    }
    None
}

fn parse_env_file(path: &PathBuf) -> Result<HashMap<String, String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read env file: {}", path.display()))?;
    let mut values = HashMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            value = value[1..value.len().saturating_sub(1)].to_string();
        }
        values.insert(key, value);
    }
    Ok(values)
}

fn required_env(values: &HashMap<String, String>, name: &str) -> Result<String> {
    values
        .get(name)
        .filter(|v| !v.trim().is_empty())
        .cloned()
        .with_context(|| format!("missing required env: {name}"))
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn config_from_env(values: &HashMap<String, String>) -> Result<String> {
    let provider = required_env(values, "AICMD_MODEL_PROVIDER")?;
    let name = required_env(values, "AICMD_MODEL_NAME")?;
    let api_base = required_env(values, "AICMD_MODEL_API_BASE")?;
    let api_key = required_env(values, "AICMD_MODEL_API_KEY")?;
    let model_ids = required_env(values, "AICMD_MODEL_IDS")?;
    let first_model_id = model_ids
        .split(',')
        .map(str::trim)
        .find(|v| !v.is_empty())
        .context("AICMD_MODEL_IDS must contain at least one model id")?;
    let mut default_model = values
        .get("AICMD_DEFAULT_MODEL")
        .cloned()
        .unwrap_or_else(|| format!("{name}:{first_model_id}"));
    if !default_model.contains(':') {
        default_model = format!("{name}:{default_model}");
    }
    let openai_api_style = values
        .get("AICMD_OPENAI_API_STYLE")
        .map(String::as_str)
        .unwrap_or("chat");
    let client_type = match provider.as_str() {
        "openai" => {
            if openai_api_style != "chat" && openai_api_style != "responses" {
                bail!("AICMD_OPENAI_API_STYLE must be chat or responses");
            }
            "openai"
        }
        "anthropic" => "claude",
        "google" => "gemini",
        _ => bail!("AICMD_MODEL_PROVIDER must be openai, anthropic, or google"),
    };

    let mut out = format!(
        r#"# AICmd runtime config / AICmd 运行时配置
# Generated from .env. Edit this file directly after installation.
# 由 .env 生成。安装后如需调整，请直接编辑本文件。

model: {default_model}

temperature: 0
top_p: null
stream: false
ai_summary: false
save: true
wrap: no
highlight: true
save_shell_history: true

document_loaders:
  pdf: 'pdftotext $1 -'
  docx: 'pandoc --to plain $1'

clients:
  - type: {client_type}
    name: {name}
    api_base: {api_base}
    api_key: {api_key}
"#,
        name = yaml_string(&name),
        api_base = yaml_string(&api_base),
        api_key = yaml_string(&api_key),
    );
    if provider == "openai" {
        out.push_str(&format!("    api_style: {openai_api_style}\n"));
    }
    out.push_str("    models:\n");
    for model_id in model_ids
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        out.push_str(&format!("      - name: {}\n", yaml_string(model_id)));
    }
    Ok(out)
}

fn default_config() -> &'static str {
    r#"# AICmd runtime config / AICmd 运行时配置
#
# Recommended: copy .env.example to .env, fill model values, then run:
#   aicmd init --from-env
#
# 推荐：复制 .env.example 为 .env，填写模型参数，然后运行：
#   aicmd init --from-env

model: openai:gpt-4o

temperature: 0
top_p: null
stream: false
ai_summary: false
save: true
wrap: no
highlight: true
save_shell_history: true

document_loaders:
  pdf: 'pdftotext $1 -'
  docx: 'pandoc --to plain $1'

clients:
  - type: openai
    name: openai
    api_base: https://api.openai.com/v1
    api_key: sk-xxxx
    api_style: chat
    models:
      - name: gpt-4o
"#
}

fn confirm_init(path: &Path, source_desc: &str, mcp_source: Option<&Path>) -> Result<()> {
    eprintln!("About to write AICmd config: {}", path.display());
    eprintln!("Source: {source_desc}");
    if let Some(mcp_source) = mcp_source {
        eprintln!(
            "MCP config will be copied: {} -> {}",
            mcp_source.display(),
            mcp_config_path().display()
        );
    }
    eprint!("Continue? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    match answer.trim() {
        "y" | "Y" | "yes" | "YES" => Ok(()),
        _ => bail!("cancelled"),
    }
}

fn sync_mcp_config(source: &Path) -> Result<Option<PathBuf>> {
    let target = mcp_config_path();
    let source_abs = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    let target_abs = target.canonicalize().unwrap_or_else(|_| target.clone());
    if source_abs == target_abs {
        return Ok(Some(target));
    }
    let content = fs::read_to_string(source)
        .with_context(|| format!("failed to read MCP config: {}", source.display()))?;
    serde_json::from_str::<serde_json::Value>(&content)
        .with_context(|| format!("invalid MCP JSON: {}", source.display()))?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o600));
    }
    Ok(Some(target))
}

fn init_config(force: bool, from_env: bool) -> Result<()> {
    let path = config_path();
    if path.exists() && !force {
        bail!(
            "config already exists: {}\nuse 'aicmd model init --force' to overwrite it",
            path.display()
        );
    }
    let mut env_source = None;
    let (content, source) = if let Some(env_file) = find_env_file() {
        let values = parse_env_file(&env_file)?;
        let content = config_from_env(&values)?;
        let source = env_file.display().to_string();
        env_source = Some(env_file);
        (content, source)
    } else if from_env {
        bail!("env file not found. Copy .env.example to .env and fill it first.");
    } else {
        (
            default_config().to_string(),
            "built-in starter config".into(),
        )
    };
    let mcp_source = env_source
        .as_deref()
        .and_then(|env_file| find_mcp_file(Some(env_file)));
    confirm_init(&path, &source, mcp_source.as_deref())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    if source == "built-in starter config" {
        println!("created starter config: {}", path.display());
    } else {
        println!("created config from env: {}", path.display());
        println!("env file: {source}");
    }
    if let Some(mcp_source) = mcp_source.as_deref() {
        if let Some(target) = sync_mcp_config(mcp_source)? {
            println!("copied MCP config: {}", target.display());
            println!("mcp file: {}", mcp_source.display());
        }
    }
    println!("edit config with: aicmd model edit");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_config_defaults_temperature_to_0() {
        let values = HashMap::from([
            ("AICMD_MODEL_PROVIDER".to_string(), "openai".to_string()),
            ("AICMD_MODEL_NAME".to_string(), "deepseek".to_string()),
            (
                "AICMD_MODEL_API_BASE".to_string(),
                "https://api.deepseek.com/v1".to_string(),
            ),
            ("AICMD_MODEL_API_KEY".to_string(), "sk-test".to_string()),
            ("AICMD_MODEL_IDS".to_string(), "deepseek-chat".to_string()),
        ]);

        let config = config_from_env(&values).expect("config should be generated from env");
        assert!(config.contains("\ntemperature: 0\n"));
        assert!(!config.contains("\ntemperature: null\n"));
        assert!(config.contains("\nai_summary: false\n"));
    }

    #[test]
    fn starter_config_defaults_temperature_to_0() {
        let config = default_config();
        assert!(config.contains("\ntemperature: 0\n"));
        assert!(!config.contains("\ntemperature: null\n"));
        assert!(config.contains("\nai_summary: false\n"));
    }
}
