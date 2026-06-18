use crate::{config::Config, doctor_cmd, model_cmd};

use anyhow::{bail, Result};
use std::{env, path::PathBuf};

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
        "doctor" => doctor_cmd::run_doctor_command(),
        _ => {
            eprintln!("Unknown config command: {cmd}");
            print_usage();
            Ok(2)
        }
    }
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
  mcp                Print mcp.json path / 输出 mcp.json 路径
  doctor             Run aicmd doctor / 运行 aicmd doctor
  help               Show this help / 显示帮助"#
    );
}
