use anyhow::{bail, Result};
use std::process::Command;

pub fn build_error_report(args: &[String]) -> Result<String> {
    let command_args = normalize_args(args)?;
    let output = Command::new(&command_args[0])
        .args(&command_args[1..])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = output.status.code().unwrap_or_default();
    Ok(format!(
        "下面这条命令执行失败或需要检查。请生成安全的 {shell} 诊断/修复命令，不要直接删除数据。\nCommand: {command}\nExit code: {status}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}\n",
        shell = crate::utils::SHELL.name,
        command = display_command(&command_args),
    ))
}

fn normalize_args(args: &[String]) -> Result<Vec<String>> {
    let mut items = args;
    if matches!(items.first().map(String::as_str), Some("err")) {
        items = &items[1..];
    }
    if matches!(items.first().map(String::as_str), Some("--")) {
        items = &items[1..];
    }
    if items.is_empty()
        || matches!(
            items.first().map(String::as_str),
            Some("help" | "-h" | "--help")
        )
    {
        bail!(usage());
    }
    Ok(items.to_vec())
}

fn display_command(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "._/:-=+".contains(ch))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn usage() -> &'static str {
    "Usage:\n  aicmd err -- <command> [args...]\n  aicmd err <command> [args...]\n\nRun a command, capture stdout/stderr/exit code, then ask AICmd to generate diagnostic or fix commands."
}
