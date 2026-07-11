mod cli;
mod client;
mod command_cache;
mod config;
mod config_cmd;
mod do_cmd;
mod doctor_cmd;
mod err_cmd;
mod function;
mod help_cmd;
mod mcp_cmd;
mod model_cmd;
mod plan_cmd;
mod render;
mod repair_cmd;
mod search_cmd;
mod session_cmd;
mod setup_cmd;
mod shell_init_cmd;
mod update_cmd;
#[macro_use]
mod utils;

#[macro_use]
extern crate log;

use crate::cli::Cli;
use crate::client::{call_chat_completions, call_chat_completions_streaming};
use crate::config::{
    ensure_parent_exists, load_env_file, Config, GlobalConfig, Input, COMMAND_SUMMARY_ROLE,
    EXPLAIN_SHELL_ROLE, MCP_SUMMARY_ROLE, SHELL_ROLE,
};
use crate::plan_cmd::{request_execution_plan, route_kind, ExecutionPlan, RouteKind};
use crate::render::render_error;
use crate::utils::*;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use fancy_regex::Regex;
use inquire::Text;
use parking_lot::RwLock;
use simplelog::{format_description, ConfigBuilder, LevelFilter, SimpleLogger, WriteLogger};
use std::{
    env,
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Read, Write},
    process::{self, Command, Stdio},
    sync::{Arc, LazyLock},
    thread,
};

#[tokio::main]
async fn main() -> Result<()> {
    load_env_file()?;
    let cli = Cli::parse();
    if let Some(code) = run_pre_config_shortcut(cli.text_args())? {
        process::exit(code);
    }
    let text = cli.text()?;
    let info_flag = cli.list_sessions;
    setup_logger()?;
    let config = Arc::new(RwLock::new(Config::init(info_flag).await?));
    if let Some(model_id) = &cli.model {
        config.write().set_model(model_id)?;
    }
    if let Some(code) = run_builtin_shortcut(&config, cli.text_args()).await? {
        process::exit(code);
    }
    if let Err(err) = run(config, cli, text).await {
        render_error(err);
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(not(windows))]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn cmd_double_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(windows)]
fn powershell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn command_with_cwd_capture(shell: &Shell, command: &str) -> String {
    let Ok(cwd_file) = env::var("AICMD_CWD_FILE") else {
        return command.to_string();
    };
    if cwd_file.is_empty() {
        return command.to_string();
    }
    #[cfg(not(windows))]
    let _ = shell;
    #[cfg(windows)]
    {
        if matches!(shell.name.as_str(), "powershell" | "pwsh") {
            return format!(
                "& {{\r\n{command}\r\n$__aicmd_status = if ($null -ne $LASTEXITCODE) {{ $LASTEXITCODE }} else {{ 0 }}\r\n(Get-Location).Path | Set-Content -LiteralPath {} -Encoding UTF8\r\nexit $__aicmd_status\r\n}}",
                powershell_single_quote(&cwd_file)
            );
        }
        format!(
            "{command}\r\nset __aicmd_status=%ERRORLEVEL%\r\ncd > {}\r\nexit /b %__aicmd_status%",
            cmd_double_quote(&cwd_file)
        )
    }
    #[cfg(not(windows))]
    format!(
        "{{\n{command}\n}}\n__aicmd_status=$?\npwd > {}\nexit $__aicmd_status",
        shell_single_quote(&cwd_file)
    )
}

fn sanitize_generated_command(command: &str) -> String {
    let mut out = command.trim().to_string();
    for marker in ["]<]", "<]"] {
        if let Some(index) = out.find(marker) {
            out.truncate(index);
            out = out.trim_end().to_string();
        }
    }
    out = remove_markdown_and_prose_from_command(&out);
    out = out.replace("find /v \"\" /c", "find /c /v \"\"");
    out = remove_leading_missing_target_exit_guard(&out);
    out
}

fn remove_markdown_and_prose_from_command(command: &str) -> String {
    command
        .lines()
        .filter(|line| keep_generated_command_line(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn keep_generated_command_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("```") {
        return false;
    }
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return true;
    }
    if contains_cjk(trimmed) && !is_shell_line_that_may_contain_cjk(trimmed) {
        return false;
    }
    true
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{3040}'..='\u{30ff}').contains(&ch)
            || ('\u{ac00}'..='\u{d7af}').contains(&ch)
    })
}

fn is_shell_line_that_may_contain_cjk(trimmed: &str) -> bool {
    let command_prefixes = [
        "echo ", "printf ", "cat ", "read ", "export ", "local ", "declare ", "typeset ",
    ];
    command_prefixes
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || trimmed.contains("<<")
        || trimmed.contains("=\"")
        || trimmed.contains("='")
}

fn remove_leading_missing_target_exit_guard(command: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?ms)\A(?:\s*#.*\n|\s*)*if\s+!\s+command\s+-v\s+(?P<cmd>[A-Za-z0-9_.+-]+)\s+.*?;\s*then\s*(?P<body>.*?)\n\s*fi\s*",
        )
        .unwrap()
    });
    let Some(caps) = RE.captures(command).ok().flatten() else {
        return command.to_string();
    };
    let Some(full_match) = caps.get(0) else {
        return command.to_string();
    };
    let cmd = caps.name("cmd").map(|m| m.as_str()).unwrap_or_default();
    let body = caps.name("body").map(|m| m.as_str()).unwrap_or_default();
    let rest = command[full_match.end()..].trim_start();
    if rest.is_empty()
        || is_install_dependency_command(cmd)
        || !body.to_lowercase().contains("exit 1")
        || !looks_like_install_command(rest)
    {
        return command.to_string();
    }
    rest.to_string()
}

fn is_install_dependency_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "brew" | "npm" | "node" | "git" | "curl" | "wget" | "python" | "python3" | "pip" | "pip3"
    )
}

fn looks_like_install_command(command: &str) -> bool {
    let command = command.to_lowercase();
    [
        "brew install",
        "npm install",
        "apt install",
        "apt-get install",
        "dnf install",
        "yum install",
        "pip install",
        "pip3 install",
        "cargo install",
        "| bash",
        "| sh",
    ]
    .iter()
    .any(|marker| command.contains(marker))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandRiskLevel {
    ReadOnly,
    ChangesSystem,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandRisk {
    level: CommandRiskLevel,
    reasons: Vec<&'static str>,
}

impl CommandRisk {
    fn label(&self) -> &'static str {
        match self.level {
            CommandRiskLevel::ReadOnly => "read-only / 只读",
            CommandRiskLevel::ChangesSystem => "changes system / 会修改系统或文件",
            CommandRiskLevel::Destructive => "destructive / 可能造成破坏",
        }
    }

    fn requires_confirmation(&self) -> bool {
        matches!(self.level, CommandRiskLevel::Destructive)
    }

    fn display(&self) -> String {
        if self.reasons.is_empty() {
            format!("Risk: {}", self.label())
        } else {
            format!("Risk: {} ({})", self.label(), self.reasons.join(", "))
        }
    }
}

fn classify_command_risk(command: &str) -> CommandRisk {
    let lower = command.to_lowercase();
    let mut level = CommandRiskLevel::ReadOnly;
    let mut reasons = Vec::new();

    let destructive_patterns = [
        ("rm -rf", "recursive force delete"),
        ("rm -fr", "recursive force delete"),
        ("mkfs", "format filesystem"),
        ("dd if=", "raw disk write/copy"),
        ("diskutil erase", "erase disk"),
        ("docker system prune", "docker prune"),
        ("git reset --hard", "discard git changes"),
        ("git clean -fd", "delete untracked files"),
        ("chmod -r", "recursive permission change"),
        ("chown -r", "recursive owner change"),
        ("drop database", "drop database"),
        ("truncate table", "truncate table"),
        ("delete from", "database delete"),
    ];
    for (pattern, reason) in destructive_patterns {
        if lower.contains(pattern) {
            level = CommandRiskLevel::Destructive;
            reasons.push(reason);
        }
    }

    let changing_patterns = [
        ("sudo ", "sudo"),
        (" >", "redirect write"),
        (">>", "append write"),
        ("tee ", "write file"),
        ("mkdir ", "create directory"),
        ("touch ", "create/update file"),
        ("mv ", "move/rename"),
        ("cp ", "copy"),
        ("rm ", "delete"),
        ("chmod ", "permission change"),
        ("chown ", "owner change"),
        ("install ", "install"),
        ("npm install", "install package"),
        ("brew install", "install package"),
        ("apt install", "install package"),
        ("apt-get install", "install package"),
        ("pip install", "install package"),
        ("curl ", "network"),
        ("wget ", "network"),
        ("docker run", "start container"),
        ("docker compose up", "start containers"),
        ("systemctl ", "service control"),
        ("launchctl ", "service control"),
    ];
    if !matches!(level, CommandRiskLevel::Destructive) {
        for (pattern, reason) in changing_patterns {
            if lower.contains(pattern) {
                level = CommandRiskLevel::ChangesSystem;
                reasons.push(reason);
            }
        }
    }

    reasons.sort_unstable();
    reasons.dedup();
    CommandRisk { level, reasons }
}

fn confirm_action(message: &str) -> Result<bool> {
    if let Ok(tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") {
        let mut tty_reader = BufReader::new(tty.try_clone()?);
        let mut tty_writer = tty;
        write!(tty_writer, "{message} [y/N] ")?;
        tty_writer.flush()?;
        let mut answer = String::new();
        tty_reader.read_line(&mut answer)?;
        return Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"));
    }

    eprint!("{message} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn default_session_name() -> String {
    let beijing = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid timezone"));
    format!("cmd-{}", beijing.format("%Y%m%d"))
}

fn run_pre_config_shortcut(args: &[String]) -> Result<Option<i32>> {
    let Some(cmd) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    match cmd {
        "init" => Ok(Some(model_cmd::run_model_command(args)?)),
        "help" => Ok(Some(help_cmd::run_help_command(&args[1..])?)),
        "model" => Ok(Some(model_cmd::run_model_command(&args[1..])?)),
        "config" => Ok(Some(config_cmd::run_config_command(&args[1..])?)),
        "setup" => Ok(Some(setup_cmd::run_setup_command(&args[1..])?)),
        "shell-init" => Ok(Some(shell_init_cmd::run_shell_init_command(&args[1..])?)),
        "doctor" => Ok(Some(doctor_cmd::run_doctor_command()?)),
        "update" => Ok(Some(update_cmd::run_update_command(&args[1..])?)),
        "session" => Ok(Some(session_cmd::run_session_command(&args[1..])?)),
        "last" => Ok(Some(session_cmd::run_last_command(&args[1..])?)),
        "search"
            if args.get(1).is_some_and(|v| {
                matches!(
                    v.as_str(),
                    "save"
                        | "list"
                        | "ls"
                        | "show"
                        | "open"
                        | "rm"
                        | "remove"
                        | "delete"
                        | "help"
                        | "-h"
                        | "--help"
                )
            }) =>
        {
            Ok(Some(search_cmd::run_search_store_command(&args[1..])?))
        }
        "mcp"
            if args
                .get(1)
                .is_some_and(|v| matches!(v.as_str(), "list" | "help" | "-h" | "--help")) =>
        {
            Ok(Some(mcp_cmd::run_mcp_command(&args[1..])?))
        }
        "mcp-raw" => Ok(Some(mcp_cmd::run_mcp_command(&args[1..])?)),
        _ => Ok(None),
    }
}

async fn run_builtin_shortcut(config: &GlobalConfig, args: &[String]) -> Result<Option<i32>> {
    let Some(cmd) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    match cmd {
        "search" => {
            if args.get(1).is_some_and(|v| v == "summarize") {
                let target = search_cmd::parse_summarize_target(&args[2..])?;
                let raw = search_cmd::load_raw_search(&target)?;
                let summary =
                    summarize_mcp_output(config, "search", &raw.query, &raw.raw_output).await?;
                let save_name = if target == "last" {
                    None
                } else {
                    Some(Some(target))
                };
                search_cmd::persist_search_result(&raw.query, &summary, save_name)?;
            } else {
                let options = search_cmd::parse_search_run_args(&args[1..])?;
                let raw_output = call_mcp_raw("search", &options.query)?;
                let raw_path = search_cmd::persist_raw_search_result(
                    &options.query,
                    &raw_output,
                    options.save_name.clone(),
                )?;
                match summarize_mcp_output(config, "search", &options.query, &raw_output).await {
                    Ok(summary) => {
                        search_cmd::persist_search_result(
                            &options.query,
                            &summary,
                            options.save_name,
                        )?;
                    }
                    Err(err) => {
                        eprintln!("Search completed, but LLM summary failed.");
                        eprintln!("Raw search saved: {}", raw_path.display());
                        eprintln!("Retry later: aicmd search summarize last");
                        return Err(err.context("Search completed but failed to summarize"));
                    }
                }
                prompt_search_follow_up(config, create_abort_signal(), Some(&options.query))
                    .await?;
            }
            Ok(Some(0))
        }
        "mcp" => {
            if args.len() < 2 {
                bail!("usage: aicmd mcp <command> [args...]");
            }
            if args[1] == "list" || args[1] == "help" || args[1] == "-h" || args[1] == "--help" {
                return Ok(Some(mcp_cmd::run_mcp_command(&args[1..])?));
            }
            let mcp_command = &args[1];
            let query = args[2..].join(" ");
            run_mcp_with_llm_summary(config, mcp_command, &query).await?;
            Ok(Some(0))
        }
        "err" => {
            let report = err_cmd::build_error_report(args)?;
            config.write().use_role(SHELL_ROLE)?;
            let default_session = default_session_name();
            config.write().use_session(Some(&default_session))?;
            let input = Input::from_str(config, &report, None);
            shell_execute(config, &SHELL, input, create_abort_signal(), None, 0).await?;
            Ok(Some(0))
        }
        "do" => {
            run_do_shortcut(config, args, create_abort_signal()).await?;
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}

fn sanitize_mcp_output_for_llm(raw: &str) -> String {
    let blocked_terms = [
        "下注",
        "赔率",
        "賠率",
        "博彩",
        "投注",
        "盘口",
        "賭",
        "bet",
        "betting",
        "bookmaker",
        "odds",
        "wager",
    ];
    let mut kept = vec![];
    let mut previous_blank = false;
    for line in raw.lines() {
        let lower = line.to_lowercase();
        if blocked_terms.iter().any(|term| lower.contains(term)) {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !previous_blank {
                kept.push(String::new());
            }
            previous_blank = true;
            continue;
        }
        previous_blank = false;
        kept.push(line.to_string());
    }
    let mut text = kept.join("\n");
    const MAX_CHARS: usize = 24_000;
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect::<String>();
        text.push_str("\n\n[内容过长，已截断]");
    }
    text
}

fn call_mcp_raw(mcp_command: &str, query: &str) -> Result<String> {
    let raw_output = mcp_cmd::call_mcp_command(mcp_command, query)
        .with_context(|| "Unable to run MCP command")?;
    if raw_output.trim().is_empty() {
        bail!("MCP command returned no output");
    }
    Ok(raw_output)
}

async fn run_mcp_with_llm_summary(
    config: &GlobalConfig,
    mcp_command: &str,
    query: &str,
) -> Result<String> {
    let raw_output = call_mcp_raw(mcp_command, query)?;
    summarize_mcp_output(config, mcp_command, query, &raw_output).await
}

async fn prompt_search_follow_up(
    config: &GlobalConfig,
    abort_signal: AbortSignal,
    query: Option<&str>,
) -> Result<()> {
    if !*IS_STDOUT_TERMINAL {
        return Ok(());
    }
    let first_letter_color = nu_ansi_term::Color::Cyan;
    let options = [
        ("s", "ave", "保存"),
        ("d", "o", "基于结果执行"),
        ("o", "pen", "打开"),
        ("q", "uit", "退出"),
    ];
    let prompt_text = options
        .iter()
        .map(|(key, rest, zh)| format!("{}{}({})", color_text(key, first_letter_color), rest, zh))
        .collect::<Vec<String>>()
        .join(&dimmed_text(" | "));
    let answer = read_single_key(&['s', 'd', 'o', 'q'], 'q', &format!("{prompt_text}: "))?;
    match answer {
        's' => {
            let name = Text::new("Save name (empty = auto):")
                .prompt()
                .unwrap_or_default();
            let name = name.trim();
            search_cmd::save_last(if name.is_empty() { None } else { Some(name) })?;
        }
        'd' => {
            let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) else {
                bail!("No search query available for do action");
            };
            println!(
                "{}",
                dimmed_text(&format!(
                    "Using search result as task / 使用搜索结果作为任务: {query}"
                ))
            );
            println!(
                "{}",
                dimmed_text(
                    "Generating execution script from search result and system environment. This may take 10-30 seconds. / 正在根据搜索结果和当前系统环境生成执行脚本，可能需要 10-30 秒。"
                )
            );
            let args = vec![
                "do".to_string(),
                "--from-search".to_string(),
                "last".to_string(),
                query.to_string(),
            ];
            run_do_shortcut(config, &args, abort_signal).await?;
        }
        'o' => {
            search_cmd::open_search(Some("last"))?;
        }
        _ => {}
    }
    Ok(())
}

async fn run_do_shortcut(
    config: &GlobalConfig,
    args: &[String],
    abort_signal: AbortSignal,
) -> Result<()> {
    let request = do_cmd::build_do_request(args, &SHELL.name)?;
    if request.dry_run {
        config.write().dry_run = true;
    }
    config.write().use_role(SHELL_ROLE)?;
    let default_session = default_session_name();
    config.write().use_session(Some(&default_session))?;
    let input = Input::from_str(config, &request.prompt, None);
    shell_execute(config, &SHELL, input, abort_signal, None, 0).await
}

async fn summarize_mcp_output(
    config: &GlobalConfig,
    mcp_command: &str,
    query: &str,
    raw_output: &str,
) -> Result<String> {
    let abort_signal = create_abort_signal();
    let llm_output = sanitize_mcp_output_for_llm(raw_output);
    if llm_output.trim().is_empty() {
        bail!("MCP command returned no usable output after filtering");
    }
    let success = true;
    let status_text = if success { "success" } else { "failed" };
    let prompt = format!(
        "MCP command: {mcp_command}
MCP status: {status_text}
User request / 用户请求：{query}

MCP returned content / MCP 返回内容：
{llm_output}"
    );
    let role = config.read().retrieve_role(MCP_SUMMARY_ROLE)?;
    let input = Input::from_str(config, &prompt, Some(role));
    let client = input.create_client()?;
    let (summary, _) = if input.stream() {
        call_chat_completions_streaming(&input, client.as_ref(), abort_signal).await?
    } else {
        call_chat_completions(&input, true, false, client.as_ref(), abort_signal).await?
    };
    println!();
    Ok(summary)
}

fn run_shell_command_capture(shell: &Shell, command: &str) -> Result<(i32, String, String)> {
    let mut child = Command::new(&shell.cmd)
        .args([&shell.arg, command])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;
    let stdout_handle = thread::spawn(move || stream_and_capture(stdout, false));
    let stderr_handle = thread::spawn(move || stream_and_capture(stderr, true));
    let status = child.wait()?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;
    let code = status.code().unwrap_or_default();
    Ok((code, stdout, stderr))
}

fn stream_and_capture<R: Read>(mut reader: R, is_stderr: bool) -> Result<String> {
    let mut captured = Vec::new();
    let mut buf = [0_u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        captured.extend_from_slice(&buf[..n]);
        write_console_chunk(&buf[..n], is_stderr)?;
    }
    Ok(decode_command_output(&captured))
}

#[cfg(windows)]
fn write_console_chunk(bytes: &[u8], is_stderr: bool) -> Result<()> {
    let text = decode_command_output(bytes);
    if is_stderr {
        io::stderr().write_all(text.as_bytes())?;
        io::stderr().flush()?;
    } else {
        io::stdout().write_all(text.as_bytes())?;
        io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(windows)]
fn decode_command_output(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let (text, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if !had_errors {
        return text.into_owned();
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(not(windows))]
fn decode_command_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(not(windows))]
fn write_console_chunk(bytes: &[u8], is_stderr: bool) -> Result<()> {
    if is_stderr {
        io::stderr().write_all(bytes)?;
        io::stderr().flush()?;
    } else {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()?;
    }
    Ok(())
}

async fn summarize_command_output(
    config: &GlobalConfig,
    command: &str,
    code: i32,
    stdout: &str,
    stderr: &str,
    abort_signal: AbortSignal,
) -> Result<Option<String>> {
    let combined = if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout.trim(), stderr.trim())
    };
    if combined.trim().is_empty() {
        return Ok(None);
    }
    let prompt = format!(
        "Executed command / 执行的命令：\n{command}\n\nExit code / 退出码：{code}\n\nCommand output / 命令输出：\n{combined}"
    );
    let role = config.read().retrieve_role(COMMAND_SUMMARY_ROLE)?;
    let input = Input::from_str(config, &prompt, Some(role));
    let client = input.create_client()?;
    println!("{}", dimmed_text("\nAI summary:"));
    let (summary, _) = if input.stream() {
        call_chat_completions_streaming(&input, client.as_ref(), abort_signal).await?
    } else {
        call_chat_completions(&input, true, false, client.as_ref(), abort_signal).await?
    };
    println!();
    Ok(Some(summary))
}

fn truncate_for_session(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push_str("\n[truncated / 已截断]");
    }
    if out.trim().is_empty() {
        "(empty)".to_string()
    } else {
        out
    }
}

fn build_execution_session_note(
    command: &str,
    code: i32,
    stdout: &str,
    stderr: &str,
    summary: Option<&str>,
) -> String {
    const OUTPUT_LIMIT: usize = 4_000;
    const SUMMARY_LIMIT: usize = 2_000;
    let stdout = truncate_for_session(stdout.trim(), OUTPUT_LIMIT);
    let stderr = truncate_for_session(stderr.trim(), OUTPUT_LIMIT);
    let summary = summary
        .map(|value| truncate_for_session(value.trim(), SUMMARY_LIMIT))
        .unwrap_or_else(|| "(empty)".to_string());
    format!(
        "Command execution result:\nCommand:\n{command}\n\nExit code: {code}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}\n\nAI summary:\n{summary}"
    )
}

async fn run(config: GlobalConfig, cli: Cli, text: Option<String>) -> Result<()> {
    let abort_signal = create_abort_signal();

    if cli.dry_run {
        config.write().dry_run = true;
    }

    if cli.print_command {
        config.write().print_command = true;
    }
    if cli.summary {
        config.write().ai_summary = true;
    }
    if cli.no_summary {
        config.write().ai_summary = false;
    }

    config.write().use_role(SHELL_ROLE)?;
    let default_session = default_session_name();
    if matches!(cli.session, Some(None)) && text.is_none() && cli.file.is_empty() {
        println!("current session: {default_session}");
        return Ok(());
    }
    let session_name = if let Some(session) = &cli.session {
        session.as_ref().map(|v| v.as_str())
    } else {
        Some(default_session.as_str())
    };
    let context_enabled = cli.session.is_some();
    config
        .write()
        .use_session_with_context(session_name, context_enabled)?;
    if cli.list_sessions {
        let sessions = config.read().list_sessions().join("\n");
        println!("{sessions}");
        return Ok(());
    }
    if cli.empty_session {
        let target = session_name.unwrap_or("temporary");
        if !confirm_action(&format!(
            "Clear all history in session '{target}'? / 确认清空会话 '{target}' 的全部历史记录？"
        ))? {
            println!("cancelled / 已取消");
            return Ok(());
        }
        config.write().empty_session()?;
        if text.is_none() && cli.file.is_empty() {
            println!("session cleared: {target}");
            return Ok(());
        }
    }
    if text.is_none() && cli.file.is_empty() {
        if cli.session.is_some() {
            if let Some(name) = config.write().save_current_session()? {
                println!("session ready: {name}");
            }
        } else {
            Cli::command().print_help()?;
            println!();
        }
        return Ok(());
    }
    let cache_task = if cli.no_cache || !cli.file.is_empty() {
        None
    } else {
        text.as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let input = create_input(&config, text, &cli.file, abort_signal.clone()).await?;
    let plan = request_execution_plan(&config, &input, abort_signal.clone())
        .await
        .context("Invalid execution plan / 无效执行计划")?;
    route_execution_plan(&config, &SHELL, input, plan, abort_signal, cache_task).await?;
    Ok(())
}

async fn route_execution_plan(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    plan: ExecutionPlan,
    abort_signal: AbortSignal,
    cache_task: Option<String>,
) -> Result<()> {
    if config.read().dry_run {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    if config.read().print_command {
        match route_kind(&plan.mode) {
            RouteKind::Command => println!("{}", plan.command),
            RouteKind::Search => println!("mode: search\nquery: {}", plan.query),
            RouteKind::Diagnose => println!("mode: diagnose\nproblem: {}", plan.problem),
        }
        return Ok(());
    }

    match route_kind(&plan.mode) {
        RouteKind::Command => {
            handle_generated_command(
                config,
                shell,
                input,
                abort_signal,
                ShellExecutionOptions {
                    eval_str: plan.command,
                    cache_task,
                    record_assistant_message: true,
                    repair_attempts: 0,
                    from_cache: false,
                },
            )
            .await
        }
        RouteKind::Search => {
            let raw_output = call_mcp_raw("search", &plan.query)?;
            summarize_mcp_output(config, "search", &plan.query, &raw_output).await?;
            prompt_search_follow_up(config, abort_signal, Some(&plan.query)).await
        }
        RouteKind::Diagnose => {
            let input = Input::from_str(config, &plan.problem, None);
            shell_execute(config, shell, input, abort_signal, None, 0).await
        }
    }
}

#[async_recursion::async_recursion]
async fn shell_execute(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    abort_signal: AbortSignal,
    cache_task: Option<String>,
    repair_attempts: u8,
) -> Result<()> {
    if let Some(task) = cache_task.as_deref() {
        if !config.read().dry_run && !config.read().print_command && *IS_STDOUT_TERMINAL {
            if let Some(record) = command_cache::lookup(task, &shell.name, env::consts::OS) {
                println!(
                    "{}",
                    dimmed_text(
                        "Reusing a previously successful command / 正在复用之前成功执行过的命令"
                    )
                );
                return handle_generated_command(
                    config,
                    shell,
                    input,
                    abort_signal,
                    ShellExecutionOptions {
                        eval_str: record.command,
                        cache_task,
                        record_assistant_message: false,
                        repair_attempts,
                        from_cache: true,
                    },
                )
                .await;
            }
        }
    }

    let client = input.create_client()?;
    config.write().before_chat_completion(&input)?;
    let (eval_str, _) =
        call_chat_completions(&input, false, true, client.as_ref(), abort_signal.clone()).await?;
    if config.read().dry_run {
        config.read().print_markdown(&eval_str)?;
        return Ok(());
    }
    handle_generated_command(
        config,
        shell,
        input,
        abort_signal,
        ShellExecutionOptions {
            eval_str,
            cache_task,
            record_assistant_message: true,
            repair_attempts,
            from_cache: false,
        },
    )
    .await
}

#[derive(Debug, Clone)]
struct ShellExecutionOptions {
    eval_str: String,
    cache_task: Option<String>,
    record_assistant_message: bool,
    repair_attempts: u8,
    from_cache: bool,
}

#[async_recursion::async_recursion]
async fn handle_generated_command(
    config: &GlobalConfig,
    shell: &Shell,
    mut input: Input,
    abort_signal: AbortSignal,
    options: ShellExecutionOptions,
) -> Result<()> {
    let ShellExecutionOptions {
        eval_str,
        cache_task,
        record_assistant_message,
        repair_attempts,
        from_cache,
    } = options;
    let eval_str = sanitize_generated_command(&eval_str);

    if record_assistant_message {
        config.write().after_chat_completion(&input, &eval_str)?;
    }
    if eval_str.is_empty() {
        bail!("No command generated");
    }
    if config.read().print_command {
        println!("{eval_str}");
        return Ok(());
    }
    let client = input.create_client()?;
    if *IS_STDOUT_TERMINAL {
        let command = color_text(eval_str.trim(), nu_ansi_term::Color::Rgb(255, 165, 0));
        let risk = classify_command_risk(&eval_str);
        loop {
            println!("{command}");
            println!("{}", dimmed_text(&risk.display()));
            let mut answer_char =
                read_single_key(&['y', 'n', '?'], 'y', "Run? [Y/n/?] / 执行？[Y/n/?] ")?;
            if answer_char == '?' {
                let first_letter_color = nu_ansi_term::Color::Cyan;
                let mut keys = vec!['r', 'd', 'c', 'q'];
                let mut options = vec![
                    format!(
                        "{}{}{}",
                        color_text("r", first_letter_color),
                        "evise",
                        "(修改)"
                    ),
                    format!(
                        "{}{}{}",
                        color_text("d", first_letter_color),
                        "escribe",
                        "(解释)"
                    ),
                    format!(
                        "{}{}{}",
                        color_text("c", first_letter_color),
                        "opy",
                        "(复制)"
                    ),
                    format!(
                        "{}{}{}",
                        color_text("q", first_letter_color),
                        "uit",
                        "(退出)"
                    ),
                ];
                if from_cache {
                    keys.insert(0, 'g');
                    options.insert(
                        0,
                        format!(
                            "{}{}{}",
                            color_text("g", first_letter_color),
                            "enerate",
                            "(重新生成)"
                        ),
                    );
                }
                answer_char = read_single_key(
                    &keys,
                    'q',
                    &format!("More / 更多：{}: ", options.join(&dimmed_text(" | "))),
                )?;
            }

            match answer_char {
                'y' => {
                    if risk.requires_confirmation()
                        && !confirm_action("High-risk command. Continue? / 高风险命令，确认执行？")?
                    {
                        println!("cancelled / 已取消");
                        continue;
                    }
                    let eval_command = command_with_cwd_capture(shell, &eval_str);
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_command]);
                    let (code, stdout, stderr) = run_shell_command_capture(shell, &eval_command)?;
                    if code == 0 && config.read().save_shell_history {
                        let _ = append_to_shell_history(&shell.name, &eval_str, code);
                    }
                    let summary = if config.read().ai_summary {
                        match summarize_command_output(
                            config,
                            &eval_str,
                            code,
                            &stdout,
                            &stderr,
                            abort_signal.clone(),
                        )
                        .await
                        {
                            Ok(summary) => summary,
                            Err(err) => {
                                eprintln!("AI summary failed: {err:#}");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let session_note = build_execution_session_note(
                        &eval_str,
                        code,
                        &stdout,
                        &stderr,
                        summary.as_deref(),
                    );
                    config.write().append_session_note(session_note)?;
                    if code == 0 {
                        if let Some(task) = cache_task.as_deref() {
                            if let Err(err) = command_cache::record_success(
                                task,
                                &shell.name,
                                env::consts::OS,
                                &eval_str,
                            ) {
                                eprintln!("Command cache update failed: {err:#}");
                            }
                        }
                    }
                    if code != 0 && *IS_STDOUT_TERMINAL {
                        loop {
                            let first_letter_color = nu_ansi_term::Color::Cyan;
                            let mut option_keys = vec!['e', 'c', 'q'];
                            let mut options = vec![
                                format!(
                                    "{}{}{}",
                                    color_text("e", first_letter_color),
                                    "xplain",
                                    "(解释)"
                                ),
                                format!(
                                    "{}{}{}",
                                    color_text("c", first_letter_color),
                                    "opy",
                                    "(复制)"
                                ),
                                format!(
                                    "{}{}{}",
                                    color_text("q", first_letter_color),
                                    "uit",
                                    "(退出)"
                                ),
                            ];
                            if repair_attempts < 2 {
                                option_keys.insert(0, 'f');
                                options.insert(
                                    0,
                                    format!(
                                        "{}{}{}",
                                        color_text("f", first_letter_color),
                                        "ix",
                                        "(修复)"
                                    ),
                                );
                            } else {
                                println!(
                                    "{}",
                                    dimmed_text(
                                        "Repair limit reached / 已达到自动修复次数上限。Please inspect the error manually or revise the task. / 请手动检查错误，或修改任务描述。"
                                    )
                                );
                            }
                            let prompt = format!(
                                "Command failed / 命令执行失败。{}: ",
                                options.join(&dimmed_text(" | "))
                            );
                            let next = read_single_key(&option_keys, 'e', &prompt)?;
                            match next {
                                'f' if repair_attempts < 2 => {
                                    let cwd = env::current_dir()
                                        .map(|path| path.display().to_string())
                                        .unwrap_or_else(|_| "unknown".to_string());
                                    let user_task = input.text();
                                    let repair_prompt = repair_cmd::build_repair_prompt(
                                        &repair_cmd::RepairContext {
                                            user_task: &user_task,
                                            shell: &shell.name,
                                            os: env::consts::OS,
                                            cwd: &cwd,
                                            command: &eval_str,
                                            exit_code: code,
                                            stdout: &stdout,
                                            stderr: &stderr,
                                        },
                                    );
                                    input.set_text(repair_prompt);
                                    return shell_execute(
                                        config,
                                        shell,
                                        input,
                                        abort_signal.clone(),
                                        None,
                                        repair_attempts + 1,
                                    )
                                    .await;
                                }
                                'e' => {
                                    if let Err(err) = summarize_command_output(
                                        config,
                                        &eval_str,
                                        code,
                                        &stdout,
                                        &stderr,
                                        abort_signal.clone(),
                                    )
                                    .await
                                    {
                                        eprintln!("Failure explanation failed: {err:#}");
                                    }
                                    continue;
                                }
                                'c' => {
                                    set_text(&eval_str)?;
                                    println!("{}", dimmed_text("✓ Copied the failed command."));
                                    continue;
                                }
                                _ => break,
                            }
                        }
                    }
                    process::exit(code);
                }
                'g' if from_cache => {
                    return shell_execute(
                        config,
                        shell,
                        input,
                        abort_signal.clone(),
                        None,
                        repair_attempts,
                    )
                    .await;
                }
                'r' => {
                    let revision = Text::new("Enter revision / 输入修改要求:").prompt()?;
                    let text = format!("{}\n{revision}", input.text());
                    input.set_text(text);
                    return shell_execute(
                        config,
                        shell,
                        input,
                        abort_signal.clone(),
                        None,
                        repair_attempts,
                    )
                    .await;
                }
                'd' => {
                    let role = config.read().retrieve_role(EXPLAIN_SHELL_ROLE)?;
                    let input = Input::from_str(config, &eval_str, Some(role));
                    if input.stream() {
                        call_chat_completions_streaming(
                            &input,
                            client.as_ref(),
                            abort_signal.clone(),
                        )
                        .await?;
                    } else {
                        call_chat_completions(
                            &input,
                            true,
                            false,
                            client.as_ref(),
                            abort_signal.clone(),
                        )
                        .await?;
                    }
                    println!();
                    continue;
                }
                'c' => {
                    set_text(&eval_str)?;
                    println!("{}", dimmed_text("✓ Copied the command."));
                    continue;
                }
                _ => break,
            }
        }
    } else {
        println!("{eval_str}");
    }
    Ok(())
}

async fn create_input(
    config: &GlobalConfig,
    text: Option<String>,
    file: &[String],
    abort_signal: AbortSignal,
) -> Result<Input> {
    let input = if file.is_empty() {
        Input::from_str(config, &text.unwrap_or_default(), None)
    } else {
        Input::from_files_with_spinner(
            config,
            &text.unwrap_or_default(),
            file.to_vec(),
            None,
            abort_signal,
        )
        .await?
    };
    if input.is_empty() {
        bail!("No input");
    }
    Ok(input)
}

fn setup_logger() -> Result<()> {
    let (log_level, log_path) = Config::log_config()?;
    if log_level == LevelFilter::Off {
        return Ok(());
    }
    let crate_name = env!("CARGO_CRATE_NAME");
    let log_filter =
        std::env::var(get_env_name("log_filter")).unwrap_or_else(|_| crate_name.into());
    let config = ConfigBuilder::new()
        .add_filter_allow(log_filter)
        .set_time_format_custom(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .set_thread_level(LevelFilter::Off)
        .build();
    match log_path {
        None => {
            SimpleLogger::init(log_level, config)?;
        }
        Some(log_path) => {
            ensure_parent_exists(&log_path)?;
            let log_file = std::fs::File::create(log_path)?;
            WriteLogger::init(log_level, config, log_file)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_for_session_marks_truncated_content() {
        let value = truncate_for_session("abcdef", 3);
        assert_eq!(value, "abc\n[truncated / 已截断]");
    }

    #[test]
    fn build_execution_session_note_includes_execution_fields() {
        let note =
            build_execution_session_note("printf hello", 0, "hello", "", Some("printed hello"));
        assert!(note.contains("Command execution result:"));
        assert!(note.contains("Command:\nprintf hello"));
        assert!(note.contains("Exit code: 0"));
        assert!(note.contains("STDOUT:\nhello"));
        assert!(note.contains("STDERR:\n(empty)"));
        assert!(note.contains("AI summary:\nprinted hello"));
    }

    #[test]
    fn sanitize_generated_command_removes_minimax_marker() {
        assert_eq!(
            sanitize_generated_command("dir /a /b | find /c /v \"\"]<]minimax[>["),
            "dir /a /b | find /c /v \"\""
        );
    }

    #[test]
    fn sanitize_generated_command_keeps_normal_command() {
        assert_eq!(
            sanitize_generated_command("wmic logicaldisk get caption,freespace,size"),
            "wmic logicaldisk get caption,freespace,size"
        );
    }

    #[test]
    fn sanitize_generated_command_fixes_windows_find_count_order() {
        assert_eq!(
            sanitize_generated_command("dir /ad /b 2>nul | find /v \"\" /c"),
            "dir /ad /b 2>nul | find /c /v \"\""
        );
    }

    #[test]
    fn sanitize_generated_command_removes_impossible_install_precheck() {
        let input = r#"# 首先检查是否已安装 Copilot CLI
if ! command -v copilot-cli &> /dev/null; then
    echo "Copilot CLI is not installed. Please install it first."
    exit 1
fi

# 安装 Copilot CLI
brew install copilot-cli
copilot --version"#;

        assert_eq!(
            sanitize_generated_command(input),
            "# 安装 Copilot CLI\nbrew install copilot-cli\ncopilot --version"
        );
    }

    #[test]
    fn sanitize_generated_command_keeps_dependency_precheck() {
        let input = r#"if ! command -v brew &> /dev/null; then
    echo "Homebrew is required."
    exit 1
fi
brew install copilot-cli"#;

        assert_eq!(sanitize_generated_command(input), input);
    }

    #[test]
    fn sanitize_generated_command_removes_markdown_fences_and_prose() {
        let input = r#"# 检查 Homebrew 是否已安装
if command -v brew >/dev/null 2>&1; then
    echo "Homebrew 已安装"
else
    echo "Homebrew 未安装，请先安装 Homebrew"
    exit 1
fi
```
如果 Homebrew 已安装，则继续执行以下步骤：
```zsh
# 安装 Copilot CLI
brew install copilot-cli@prerelease"#;

        assert_eq!(
            sanitize_generated_command(input),
            "# 检查 Homebrew 是否已安装\nif command -v brew >/dev/null 2>&1; then\n    echo \"Homebrew 已安装\"\nelse\n    echo \"Homebrew 未安装，请先安装 Homebrew\"\n    exit 1\nfi\n# 安装 Copilot CLI\nbrew install copilot-cli@prerelease"
        );
    }
}
