use crate::{config::Config, model_cmd};

use anyhow::{bail, Result};
use std::{env, path::PathBuf};

pub fn run_setup_command(args: &[String]) -> Result<i32> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "-h" | "--help"))
    {
        print_usage();
        return Ok(0);
    }
    if !args.is_empty() {
        bail!("Unknown option for setup: {}", args.join(" "));
    }

    let config_path = Config::config_file();
    let config_dir = Config::config_dir();
    let env_path = find_env_file();
    let mcp_path = mcp_config_path();

    println!("AICmd setup");
    println!();
    println!("Config dir: {}", config_dir.display());
    println!("Config file: {}", config_path.display());
    println!("MCP file: {}", mcp_path.display());
    println!();

    if config_path.exists() {
        println!("Config: exists / 已存在");
        println!("Next checks / 下一步检查:");
        println!("  aicmd doctor");
        println!("  aicmd 当前目录有多少文件");
        println!();
        println!("To regenerate from .env / 如需从 .env 重新生成:");
        println!("  aicmd init --from-env --force");
        return Ok(0);
    }

    match env_path {
        Some(path) => {
            println!("Found env file / 找到 .env: {}", path.display());
            println!("AICmd can generate config.yaml from this file.");
            println!("AICmd 可以基于这个文件生成 config.yaml。");
            println!();
            model_cmd::run_model_command(&["init".into(), "--from-env".into()])?;
            println!();
            println!("Recommended next step / 建议下一步:");
            println!("  aicmd doctor");
            Ok(0)
        }
        None => {
            println!("Config: missing / 不存在");
            println!("Env file: not found / 未找到 .env");
            println!();
            println!("First-time setup / 首次配置:");
            println!("1. Create a .env file with your model settings.");
            println!("   创建包含模型配置的 .env 文件。");
            println!("2. Run: aicmd setup");
            println!("   或运行: aicmd init --from-env");
            println!("3. Check: aicmd doctor");
            println!("4. Try: aicmd 当前目录有多少文件");
            Ok(0)
        }
    }
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

fn mcp_config_path() -> PathBuf {
    env::var("AICMD_MCP_CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Config::config_dir().join("mcp.json"))
}

fn print_usage() {
    println!(
        r#"Usage: aicmd setup

Prepare AICmd for first use.

用法：aicmd setup

为首次使用 AICmd 做准备。

What it does / 功能:
  - Shows config paths / 显示配置路径
  - Finds .env / 查找 .env
  - Generates config.yaml from .env / 从 .env 生成 config.yaml
  - Copies mcp.json when init finds one / 初始化时复制 mcp.json
  - Suggests aicmd doctor / 建议运行 aicmd doctor"#
    );
}
