use anyhow::{bail, Context, Result};
use chrono::Local;
use std::{env, fs, path::PathBuf, process::Command};

use crate::{search_cmd, utils::strip_ansi_codes};

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
    let has_search_context = !search_refs.is_empty();
    let file_context = read_file_context(&resolve_context_files(&files, &search_refs)?)?;
    let script_kind = script_kind(shell_name);
    let script_path = output.unwrap_or_else(|| default_script_path(script_kind.extension));
    let system_context = if has_search_context && !plan {
        build_system_context(shell_name)
    } else {
        String::new()
    };
    let prompt = build_prompt(
        &task,
        &file_context,
        &system_context,
        script_kind,
        &script_path,
        plan,
        has_search_context,
    );
    Ok(DoRequest { dry_run, prompt })
}

fn build_prompt(
    task: &str,
    file_context: &str,
    system_context: &str,
    script_kind: ScriptKind,
    script_path: &str,
    plan: bool,
    has_search_context: bool,
) -> String {
    let prompt = if plan {
        format!(
            "为这个任务制定执行计划，不要创建脚本，不要执行实际任务，不要安装软件，不要修改文件或系统状态。任务: {task}。请只输出一条安全的 {kind} 命令，用 cat <<'EOF' 或 printf 打印中文计划。计划必须包含：目标、准备检查、执行步骤、风险/权限、验证方式、下一步建议。如果任务信息不足，请在计划中说明缺少什么。{file_context}",
            kind = script_kind.display,
            task = task,
            file_context = file_context,
        )
    } else if has_search_context {
        format!(
            "根据参考搜索结果和当前系统环境，创建一个可审查的 {kind} 脚本 {path} 来完成这个任务: {task}。要求：只输出一条可直接执行的终端命令；这条命令必须创建脚本、写入脚本内容、设置可执行权限并执行脚本；不要输出 markdown 代码块、解释段落或自然语言步骤。脚本要求：使用 shebang；打印清晰步骤；先根据当前系统环境检查必要依赖；优先使用搜索结果中的官方或直接来源；安装或修改系统状态前选择可审查、可确认的命令；安装/设置软件时，不要因为目标命令当前不存在就退出，因为安装它正是任务目标；可以先检查 brew/npm/node/git/curl 等依赖，或使用 `if command -v 目标命令 >/dev/null 2>&1; then 目标命令 --version; else 安装命令 && 目标命令 --version; fi` 这种幂等结构；如果依赖的包管理器不存在，不要调用不存在的包管理器安装自己；安装后必须包含验证步骤；如果搜索结果不足、命令不安全、需要用户登录/凭据/付费权限，或无法确定正确安装方式，请不要创建会修改系统的脚本，只输出一条安全说明命令解释原因和下一步建议。{system_context}{file_context}",
            kind = script_kind.display,
            path = script_path,
            task = task,
            system_context = system_context,
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
    prompt
}

fn build_system_context(shell_name: &str) -> String {
    let mut out = String::from("\n\n当前系统环境 / Current system environment:\n");
    out.push_str(&format!("shell: {shell_name}\n"));
    out.push_str(&format!("os: {}\n", env::consts::OS));
    out.push_str(&format!("arch: {}\n", env::consts::ARCH));
    if let Ok(dir) = env::current_dir() {
        out.push_str(&format!("cwd: {}\n", dir.display()));
    }
    for (name, command) in [
        ("uname", "uname -a 2>/dev/null || true"),
        ("brew", "command -v brew >/dev/null 2>&1 && { command -v brew; brew --version | head -n 1; } || echo missing"),
        ("node", "command -v node >/dev/null 2>&1 && { command -v node; node --version; } || echo missing"),
        ("npm", "command -v npm >/dev/null 2>&1 && { command -v npm; npm --version; } || echo missing"),
        ("git", "command -v git >/dev/null 2>&1 && { command -v git; git --version; } || echo missing"),
        ("curl", "command -v curl >/dev/null 2>&1 && command -v curl || echo missing"),
        ("wget", "command -v wget >/dev/null 2>&1 && command -v wget || echo missing"),
    ] {
        out.push_str(&format!("{name}: {}\n", run_probe(command)));
    }
    out
}

fn run_probe(command: &str) -> String {
    let output = Command::new("sh").arg("-lc").arg(command).output();
    let Ok(output) = output else {
        return "unknown".to_string();
    };
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    if text.trim().is_empty() {
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let text = strip_ansi_codes(&text);
    let mut text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" | ");
    const MAX_LEN: usize = 240;
    if text.chars().count() > MAX_LEN {
        text = text.chars().take(MAX_LEN).collect::<String>();
        text.push_str("...");
    }
    if text.is_empty() {
        "unknown".to_string()
    } else {
        text
    }
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
        let content = strip_ansi_codes(&content);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_context_prompt_does_not_force_script_creation() {
        let prompt = build_prompt(
            "如何安装 copilot-cli",
            "\n\n参考文件内容 / Reference file contents:\n官方安装方式...",
            "\n\n当前系统环境 / Current system environment:\nbrew: /opt/homebrew/bin/brew | Homebrew 4.5.0\n",
            script_kind("zsh"),
            ".aicmd/task-test.sh",
            false,
            true,
        );

        assert!(prompt.contains("根据参考搜索结果"));
        assert!(prompt.contains("创建一个可审查的 zsh 脚本 .aicmd/task-test.sh"));
        assert!(prompt.contains("当前系统环境"));
        assert!(prompt.contains("不要因为目标命令当前不存在就退出"));
        assert!(prompt.contains("设置可执行权限并执行脚本"));
    }

    #[test]
    fn normal_do_prompt_still_creates_script() {
        let prompt = build_prompt(
            "处理 input.csv",
            "",
            "",
            script_kind("zsh"),
            ".aicmd/task-test.sh",
            false,
            false,
        );

        assert!(prompt.contains("创建一个 zsh 脚本"));
    }

    #[test]
    fn system_context_includes_core_environment_fields() {
        let context = build_system_context("zsh");
        assert!(context.contains("当前系统环境"));
        assert!(context.contains("shell: zsh"));
        assert!(context.contains("os:"));
        assert!(context.contains("arch:"));
    }
}
