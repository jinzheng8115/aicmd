use anyhow::{bail, Context, Result};
use chrono::Local;
use std::{fs, path::PathBuf};

use crate::search_cmd;

pub struct DoRequest {
    pub dry_run: bool,
    pub prompt: String,
}

pub fn build_do_request(args: &[String], shell_name: &str) -> Result<DoRequest> {
    let mut dry_run = false;
    let mut plan = false;
    let mut output: Option<String> = None;
    let mut files: Vec<String> = vec![];
    let mut search_refs: Vec<String> = vec![];
    let mut task_parts = vec![];
    let mut i = if matches!(args.first().map(String::as_str), Some("do")) {
        1
    } else {
        0
    };
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "--plan" => {
                plan = true;
                i += 1;
            }
            "-o" | "--output" => {
                let Some(value) = args.get(i + 1) else {
                    bail!("--output requires a path");
                };
                output = Some(value.clone());
                i += 2;
            }
            "-f" | "--file" => {
                let Some(value) = args.get(i + 1) else {
                    bail!("--file requires a path");
                };
                files.push(value.clone());
                i += 2;
            }
            "--from-search" => {
                let Some(value) = args.get(i + 1) else {
                    bail!("--from-search requires a saved search name");
                };
                search_refs.push(value.clone());
                i += 2;
            }
            "-h" | "--help" => bail!(usage()),
            _ if arg.starts_with("--output=") => {
                output = Some(arg.trim_start_matches("--output=").to_string());
                i += 1;
            }
            _ if arg.starts_with("--file=") => {
                files.push(arg.trim_start_matches("--file=").to_string());
                i += 1;
            }
            _ if arg.starts_with("--from-search=") => {
                search_refs.push(arg.trim_start_matches("--from-search=").to_string());
                i += 1;
            }
            _ if arg.starts_with("--") => bail!("Unknown option: {arg}"),
            _ => {
                task_parts.push(arg.clone());
                i += 1;
            }
        }
    }
    if task_parts.is_empty() {
        bail!(usage());
    }
    let task = task_parts.join(" ");
    let file_context = read_file_context(&resolve_context_files(&files, &search_refs)?)?;
    let script_kind = script_kind(shell_name);
    let script_path = output.unwrap_or_else(|| default_script_path(script_kind.extension));
    let prompt = if plan {
        format!(
            "为这个任务制定执行计划，不要创建脚本，不要执行实际任务，不要安装软件，不要修改文件或系统状态。任务: {task}。请只输出一条安全的 {kind} 命令，用 cat <<'EOF' 或 printf 打印中文计划。计划必须包含：目标、准备检查、执行步骤、风险/权限、验证方式、下一步建议。如果任务信息不足，请在计划中说明缺少什么。{file_context}",
            kind = script_kind.display,
            task = task,
            file_context = file_context,
        )
    } else {
        format!(
            "创建一个 {kind} 脚本 {path} 来完成这个任务: {task}。要求：先检查输入文件是否存在；必要时创建输出目录；不要删除或覆盖原始文件，除非任务明确要求；脚本写入后设置为可执行或可运行；最后执行这个脚本。如果任务缺少必要信息、无法安全完成、不适合本地脚本、依赖不可用的凭据或服务，或者找不到合适的实现方式，不要硬写脚本；请只输出一条安全的说明命令，解释无法执行的原因，并告诉用户需要补充什么或下一步建议。{file_context}",
            kind = script_kind.display,
            path = script_path,
            task = task,
            file_context = file_context,
        )
    };
    Ok(DoRequest { dry_run, prompt })
}

fn resolve_context_files(files: &[String], search_refs: &[String]) -> Result<Vec<String>> {
    let mut resolved = files.to_vec();
    for name in search_refs {
        if name.trim().is_empty() {
            bail!("--from-search requires a non-empty saved search name");
        }
        let summary_path = search_cmd::saved_search_path(name)?;
        if summary_path.is_file() {
            resolved.push(summary_path.display().to_string());
            continue;
        }
        let raw_path = search_cmd::raw_search_path(name)?;
        if raw_path.is_file() {
            bail!(
                "Saved search summary not found: {name}. Raw search exists at {}. Run: aicmd search summarize {name}",
                raw_path.display()
            );
        }
        bail!("Saved search not found: {name}. Run `aicmd search list` to see saved searches.");
    }
    Ok(resolved)
}

fn read_file_context(files: &[String]) -> Result<String> {
    if files.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::from("\n\n参考文件内容 / Reference file contents:\n");
    for file in files {
        let path = PathBuf::from(file);
        if !path.is_file() {
            bail!("--file only supports regular files for aicmd do: {file}");
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read --file for aicmd do: {file}"))?;
        out.push_str(&format!(
            "\n--- FILE: {file} ---\n{content}\n--- END FILE: {file} ---\n"
        ));
    }
    Ok(out)
}

struct ScriptKind {
    display: &'static str,
    extension: &'static str,
}

fn script_kind(shell_name: &str) -> ScriptKind {
    match shell_name {
        "powershell" | "pwsh" => ScriptKind {
            display: "PowerShell",
            extension: "ps1",
        },
        _ => ScriptKind {
            display: "zsh",
            extension: "sh",
        },
    }
}

fn default_script_path(extension: &str) -> String {
    format!(
        ".aicmd/task-{}.{}",
        Local::now().format("%Y%m%d-%H%M%S"),
        extension
    )
}

fn usage() -> &'static str {
    "Usage: aicmd do [--dry-run] [--plan] [-f FILE] [--from-search NAME] [--output PATH] <task>\n\nAsk AICmd to generate shell commands that create a script for a local file/data task, then run it after AICmd's normal confirmation flow."
}
