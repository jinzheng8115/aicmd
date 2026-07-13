mod change_report_cmd;
mod cli;
mod client;
mod command_cache;
mod config;
mod config_cmd;
mod confirm_cmd;
mod do_cmd;
mod doctor_cmd;
mod err_cmd;
mod execute_cmd;
mod function;
mod help_cmd;
mod intent_cmd;
mod interactive_cmd;
mod mcp_cmd;
mod model_cmd;
mod plan_cmd;
mod preflight_cmd;
mod render;
mod repair_cmd;
mod result_cmd;
mod search_cmd;
mod session_cmd;
mod setup_cmd;
mod shell_init_cmd;
mod update_cmd;
mod workflow_cmd;
#[macro_use]
mod utils;

#[macro_use]
extern crate log;

use crate::cli::Cli;
use crate::client::{
    call_chat_completions_controlled, call_chat_completions_raw_controlled,
    call_chat_completions_streaming_controlled,
};
use crate::config::{
    ensure_parent_exists, load_env_file, Config, GlobalConfig, Input, COMMAND_SUMMARY_ROLE,
    EXPLAIN_SHELL_ROLE, MCP_SUMMARY_ROLE, SHELL_COMMAND_ROLE, SHELL_ROLE, TEMP_SESSION_NAME,
};
use crate::intent_cmd::NaturalIntent;
use crate::plan_cmd::{
    parse_generated_command, render_execution_plan, request_execution_plan, route_kind,
    ExecutionPlan, RouteKind, WorkflowFailurePolicy, WorkflowPlan, WorkflowRisk, WorkflowStepKind,
};
use crate::preflight_cmd::PreflightCheck;
use crate::render::render_error;
use crate::utils::*;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use inquire::Text;
use is_terminal::IsTerminal;
use parking_lot::RwLock;
use simplelog::{format_description, ConfigBuilder, LevelFilter, SimpleLogger, WriteLogger};
use std::{env, io, process, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    let mut cli = Cli::parse();
    if interactive_cmd::should_start(&cli, io::stdin().is_terminal(), io::stdout().is_terminal()) {
        process::exit(interactive_cmd::run(&default_session_name())?);
    }
    load_env_file()?;
    let parsed_intent = intent_cmd::parse(cli.text_args())?;
    apply_intent_cli_overrides(&mut cli, parsed_intent.as_ref());
    let natural_intent = parsed_intent
        .as_ref()
        .filter(|intent| should_run_session_intent(&cli, Some(intent)));
    if let Some(code) = run_pre_config_intent(natural_intent)? {
        process::exit(code);
    }
    if let Some(code) = run_pre_config_shortcut(cli.text_args()).await? {
        process::exit(code);
    }
    let text = match translate_session_intent(&mut cli, natural_intent) {
        Some(text) => text,
        None => cli.text()?,
    };
    let info_flag = cli.list_sessions;
    setup_logger()?;
    let config = Arc::new(RwLock::new(Config::init(info_flag).await?));
    if let Some(model_id) = &cli.model {
        config.write().set_model(model_id)?;
    }
    if let Some(code) = run_builtin_intent(&config, natural_intent).await? {
        process::exit(code);
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

fn apply_intent_cli_overrides(cli: &mut Cli, intent: Option<&NaturalIntent>) {
    if matches!(intent, Some(NaturalIntent::ContinueLastFailure)) {
        cli.no_cache = true;
    }
}

fn translate_session_intent(
    cli: &mut Cli,
    intent: Option<&NaturalIntent>,
) -> Option<Option<String>> {
    if !should_run_session_intent(cli, intent) {
        return None;
    }
    match intent {
        Some(NaturalIntent::ClearSession { name }) => {
            cli.session = name.clone().map(Some);
            cli.empty_session = true;
            Some(None)
        }
        Some(NaturalIntent::RunInSession { name, task }) => {
            cli.session = Some(Some(name.clone()));
            Some(Some(task.clone()))
        }
        Some(NaturalIntent::ContinueLastFailure) => {
            cli.session = Some(Some(default_session_name()));
            Some(Some(cli.text_args().join(" ").trim().to_string()))
        }
        _ => None,
    }
}

fn should_run_session_intent(cli: &Cli, intent: Option<&NaturalIntent>) -> bool {
    let is_session_intent = matches!(
        intent,
        Some(
            NaturalIntent::ShowRecentContext { .. }
                | NaturalIntent::CurrentSession
                | NaturalIntent::ListSessions
                | NaturalIntent::ShowSessionRecent { .. }
                | NaturalIntent::ClearSession { .. }
                | NaturalIntent::RunInSession { .. }
                | NaturalIntent::ContinueLastFailure
        )
    );
    !is_session_intent || !(cli.session.is_some() || cli.empty_session || cli.list_sessions)
}

fn run_pre_config_intent(intent: Option<&NaturalIntent>) -> Result<Option<i32>> {
    match intent {
        Some(NaturalIntent::SaveLastSearch { name }) => {
            Ok(Some(search_cmd::save_last(name.as_deref())?))
        }
        Some(NaturalIntent::ShowRecentContext { limit }) => {
            let args = vec!["show".to_string(), "--limit".to_string(), limit.to_string()];
            Ok(Some(session_cmd::run_session_command(&args)?))
        }
        Some(NaturalIntent::CurrentSession) => Ok(Some(session_cmd::run_session_command(&[])?)),
        Some(NaturalIntent::ListSessions) => {
            let args = vec!["list".to_string()];
            Ok(Some(session_cmd::run_session_command(&args)?))
        }
        Some(NaturalIntent::ShowSessionRecent { name, limit }) => {
            let args = vec![
                "show".to_string(),
                name.clone(),
                "--limit".to_string(),
                limit.to_string(),
            ];
            Ok(Some(session_cmd::run_session_command(&args)?))
        }
        _ => Ok(None),
    }
}

async fn run_builtin_intent(
    config: &GlobalConfig,
    intent: Option<&NaturalIntent>,
) -> Result<Option<i32>> {
    let Some(NaturalIntent::DoFromLastSearch { task }) = intent else {
        return Ok(None);
    };
    let args = vec![
        "do".to_string(),
        "--from-search".to_string(),
        "last".to_string(),
        task.clone(),
    ];
    run_do_shortcut(config, &args, create_abort_signal()).await?;
    Ok(Some(0))
}

fn default_session_name() -> String {
    let beijing = chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid timezone"));
    format!("cmd-{}", beijing.format("%Y%m%d"))
}

async fn run_pre_config_shortcut(args: &[String]) -> Result<Option<i32>> {
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
        "mcp-raw" => {
            let Some(command) = args.get(1) else {
                return Ok(Some(mcp_cmd::run_mcp_command(&[])?));
            };
            if matches!(command.as_str(), "list" | "help" | "-h" | "--help") {
                return Ok(Some(mcp_cmd::run_mcp_command(&args[1..])?));
            }
            let output = call_mcp_raw(
                command,
                &args[2..].join(" "),
                create_abort_signal(),
                &RetryBudget::default(),
            )
            .await?;
            print!("{output}");
            Ok(Some(0))
        }
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
                let summary = summarize_mcp_output(
                    config,
                    "search",
                    &raw.query,
                    &raw.raw_output,
                    create_abort_signal(),
                    &RetryBudget::default(),
                )
                .await?;
                let save_name = if target == "last" {
                    None
                } else {
                    Some(Some(target))
                };
                search_cmd::persist_search_result(&raw.query, &summary, save_name)?;
            } else {
                let options = search_cmd::parse_search_run_args(&args[1..])?;
                let abort_signal = create_abort_signal();
                let retry_budget = RetryBudget::default();
                let raw_output = call_mcp_raw(
                    "search",
                    &options.query,
                    abort_signal.clone(),
                    &retry_budget,
                )
                .await?;
                let raw_path = search_cmd::persist_raw_search_result(
                    &options.query,
                    &raw_output,
                    options.save_name.clone(),
                )?;
                match summarize_mcp_output(
                    config,
                    "search",
                    &options.query,
                    &raw_output,
                    abort_signal,
                    &retry_budget,
                )
                .await
                {
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
            config.write().use_role(SHELL_COMMAND_ROLE)?;
            let default_session = default_session_name();
            config.write().use_session(Some(&default_session))?;
            let input = Input::from_str(config, &report, None);
            shell_execute(
                config,
                &SHELL,
                input,
                create_abort_signal(),
                RetryBudget::default(),
                None,
                0,
            )
            .await?;
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

async fn call_mcp_raw(
    mcp_command: &str,
    query: &str,
    abort_signal: AbortSignal,
    retry_budget: &RetryBudget,
) -> Result<String> {
    let stage_zh = format!("正在调用 MCP {mcp_command}");
    let stage_en = format!("Calling MCP {mcp_command}");
    let raw_output = run_external_with_managed_retry(
        ProgressStage::new(&stage_zh, &stage_en),
        retry_budget,
        abort_signal.clone(),
        |attempt| {
            let mcp_command = mcp_command.to_string();
            let query = query.to_string();
            let abort_signal = abort_signal.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    mcp_cmd::call_mcp_command_controlled(
                        &mcp_command,
                        &query,
                        attempt.timeout,
                        abort_signal,
                    )
                })
                .await
                .context("MCP worker failed")?
            }
        },
    )
    .await
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
    let abort_signal = create_abort_signal();
    let retry_budget = RetryBudget::default();
    let raw_output = call_mcp_raw(mcp_command, query, abort_signal.clone(), &retry_budget).await?;
    summarize_mcp_output(
        config,
        mcp_command,
        query,
        &raw_output,
        abort_signal,
        &retry_budget,
    )
    .await
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
        .map(|(key, rest, zh)| {
            format!(
                "{}{}",
                color_text(key, first_letter_color),
                localized(&format!(" {zh}"), rest)
            )
        })
        .collect::<Vec<String>>()
        .join(&dimmed_text(" | "));
    let answer = confirm_cmd::read_action(&['s', 'd', 'o', 'q'], 'q', &format!("{prompt_text}: "))?;
    match answer {
        's' => {
            let name = Text::new(localized(
                "保存名称（留空为自动生成）:",
                "Save name (empty = auto):",
            ))
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
                    "{}: {query}",
                    localized("使用搜索结果作为任务", "Using search result as task")
                ))
            );
            println!(
                "{}",
                dimmed_text(localized(
                    "正在根据搜索结果和当前系统环境生成执行脚本，可能需要 10-30 秒。",
                    "Generating an execution script from the search result and system environment. This may take 10-30 seconds."
                ))
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
    config.write().use_role(SHELL_COMMAND_ROLE)?;
    let default_session = default_session_name();
    config.write().use_session(Some(&default_session))?;
    let input = Input::from_str(config, &request.prompt, None);
    shell_execute(
        config,
        &SHELL,
        input,
        abort_signal,
        RetryBudget::default(),
        None,
        0,
    )
    .await
}

async fn summarize_mcp_output(
    config: &GlobalConfig,
    mcp_command: &str,
    query: &str,
    raw_output: &str,
    abort_signal: AbortSignal,
    retry_budget: &RetryBudget,
) -> Result<String> {
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
        call_chat_completions_streaming_controlled(
            &input,
            client.as_ref(),
            abort_signal,
            retry_budget,
            ProgressStage::new("正在整理搜索结果", "Summarizing search result"),
        )
        .await?
    } else {
        call_chat_completions_controlled(
            &input,
            true,
            false,
            client.as_ref(),
            abort_signal,
            retry_budget,
            ProgressStage::new("正在整理搜索结果", "Summarizing search result"),
        )
        .await?
    };
    println!();
    Ok(summary)
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
    let retry_budget = RetryBudget::default();
    let (summary, _) = if input.stream() {
        call_chat_completions_streaming_controlled(
            &input,
            client.as_ref(),
            abort_signal,
            &retry_budget,
            ProgressStage::new("正在生成执行结果总结", "Summarizing command output"),
        )
        .await?
    } else {
        call_chat_completions_controlled(
            &input,
            true,
            false,
            client.as_ref(),
            abort_signal,
            &retry_budget,
            ProgressStage::new("正在生成执行结果总结", "Summarizing command output"),
        )
        .await?
    };
    println!();
    Ok(Some(summary))
}

async fn run(config: GlobalConfig, cli: Cli, text: Option<String>) -> Result<()> {
    let abort_signal = create_abort_signal();
    let ask_summary = should_ask_for_summary(config.read().ai_summary, cli.summary, cli.no_summary);
    config.write().ask_summary = ask_summary;

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
    if matches!(cli.session, Some(None))
        && !cli.empty_session
        && !cli.list_sessions
        && text.is_none()
        && cli.file.is_empty()
    {
        println!("current session: {default_session}");
        return Ok(());
    }
    let session_name = if let Some(session) = &cli.session {
        session.as_ref().map(|v| v.as_str())
    } else {
        Some(default_session.as_str())
    };
    let context_enabled = cli.session.is_some();
    let clear_target = cli
        .empty_session
        .then(|| session_name.unwrap_or(TEMP_SESSION_NAME));
    if let Some(target) = clear_target {
        if !confirm_cmd::confirm_high_risk(&format!(
            "{} '{target}'?",
            localized("确认清空会话的全部历史记录", "Clear all history in session")
        ))? {
            println!("{}", localized("已取消", "cancelled"));
            return Ok(());
        }
        let target_path = config.read().session_file(target);
        if !target_path.exists() {
            bail!("Session not found: {target} ({})", target_path.display());
        }
    }
    if let Some(target) = clear_target {
        config
            .write()
            .use_existing_session_with_context(target, context_enabled)?;
    } else {
        config
            .write()
            .use_session_with_context(session_name, context_enabled)?;
    }
    if cli.list_sessions {
        let sessions = config.read().list_sessions().join("\n");
        println!("{sessions}");
        return Ok(());
    }
    if cli.empty_session {
        let target = clear_target.expect("clear target is resolved above");
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
    let cache_eligible = should_lookup_command_cache(&config, cache_task.as_deref());
    let cached_command = if cache_eligible {
        cache_task
            .as_deref()
            .and_then(|task| command_cache::lookup(task, &SHELL.name, env::consts::OS))
    } else {
        None
    };
    if matches!(
        plan_request_decision(cache_eligible, cached_command.is_some()),
        PlanRequestDecision::UseCachedCommand
    ) {
        println!(
            "{}",
            dimmed_text(localized(
                "正在复用之前成功执行过的命令",
                "Reusing a previously successful command"
            ))
        );
        let ask_summary = config.read().ask_summary;
        let cached = cached_command.expect("cache hit has a record");
        return handle_generated_command(
            &config,
            &SHELL,
            input_with_execution_role(&config, input, RouteKind::Command)?,
            abort_signal,
            ShellExecutionOptions {
                eval_str: cached.command,
                preflight: cached.preflight,
                cache_task,
                record_assistant_message: false,
                repair_attempts: 0,
                from_cache: true,
                ask_summary,
            },
        )
        .await;
    }
    let retry_budget = RetryBudget::default();
    request_and_route_execution_plan(
        &config,
        &SHELL,
        input,
        abort_signal,
        retry_budget,
        cache_task,
    )
    .await?;
    Ok(())
}

fn should_ask_for_summary(ai_summary: bool, summary: bool, no_summary: bool) -> bool {
    !ai_summary && !summary && !no_summary
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanRequestDecision {
    UseCachedCommand,
    RequestPlan,
}

fn should_lookup_command_cache(config: &GlobalConfig, cache_task: Option<&str>) -> bool {
    cache_task.is_some()
        && !config.read().dry_run
        && !config.read().print_command
        && *IS_STDOUT_TERMINAL
}

fn plan_request_decision(cache_eligible: bool, cache_hit: bool) -> PlanRequestDecision {
    if cache_eligible && cache_hit {
        PlanRequestDecision::UseCachedCommand
    } else {
        PlanRequestDecision::RequestPlan
    }
}

fn command_role_for_route(route: RouteKind) -> Option<&'static str> {
    match route {
        RouteKind::Command | RouteKind::Diagnose => Some(SHELL_COMMAND_ROLE),
        RouteKind::Search | RouteKind::Workflow => None,
    }
}

fn input_with_execution_role(
    config: &GlobalConfig,
    input: Input,
    route: RouteKind,
) -> Result<Input> {
    let Some(role_name) = command_role_for_route(route) else {
        return Ok(input);
    };
    let role = config.read().retrieve_role(role_name)?;
    Ok(input.with_role(role))
}

#[async_recursion::async_recursion]
async fn request_and_route_execution_plan(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    abort_signal: AbortSignal,
    retry_budget: RetryBudget,
    cache_task: Option<String>,
) -> Result<()> {
    let plan = request_execution_plan(config, &input, abort_signal.clone(), &retry_budget)
        .await
        .context(localized("无效执行计划", "Invalid execution plan"))?;
    route_execution_plan(
        config,
        shell,
        input,
        plan,
        abort_signal,
        retry_budget,
        cache_task,
    )
    .await
}

async fn route_execution_plan(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    plan: ExecutionPlan,
    abort_signal: AbortSignal,
    retry_budget: RetryBudget,
    cache_task: Option<String>,
) -> Result<()> {
    if config.read().dry_run {
        println!("{}", render_execution_plan(&plan)?);
        return Ok(());
    }

    if config.read().print_command {
        match route_kind(&plan.mode) {
            RouteKind::Command => println!("{}", plan.command),
            RouteKind::Search => println!("mode: search\nquery: {}", plan.query),
            RouteKind::Diagnose => println!("mode: diagnose\nproblem: {}", plan.problem),
            RouteKind::Workflow => println!("{}", render_execution_plan(&plan)?),
        }
        return Ok(());
    }

    let route = route_kind(&plan.mode);
    match route {
        RouteKind::Command => {
            let ask_summary = config.read().ask_summary;
            handle_generated_command(
                config,
                shell,
                input_with_execution_role(config, input, route)?,
                abort_signal,
                ShellExecutionOptions {
                    eval_str: plan.command,
                    preflight: plan.preflight,
                    cache_task,
                    record_assistant_message: true,
                    repair_attempts: 0,
                    from_cache: false,
                    ask_summary,
                },
            )
            .await
        }
        RouteKind::Search => {
            let raw_output =
                call_mcp_raw("search", &plan.query, abort_signal.clone(), &retry_budget).await?;
            summarize_mcp_output(
                config,
                "search",
                &plan.query,
                &raw_output,
                abort_signal.clone(),
                &retry_budget,
            )
            .await?;
            prompt_search_follow_up(config, abort_signal, Some(&plan.query)).await
        }
        RouteKind::Diagnose => {
            let input = input.with_text(plan.problem).with_session_context();
            let input = input_with_execution_role(config, input, route)?;
            shell_execute(config, shell, input, abort_signal, retry_budget, None, 0).await
        }
        RouteKind::Workflow => {
            let workflow = plan.workflow().context("workflow payload missing")?;
            run_workflow_plan(config, shell, input.text(), workflow, abort_signal, 0).await
        }
    }
}

async fn run_workflow_plan(
    config: &GlobalConfig,
    shell: &Shell,
    request: String,
    plan: WorkflowPlan,
    abort_signal: AbortSignal,
    repair_attempts: u8,
) -> Result<()> {
    let mut check_results = Vec::new();
    let run_result: Result<(workflow_cmd::WorkflowStatus, &'static str, bool)> = async {
        let checks = plan
            .steps
            .iter()
            .take_while(|step| step.kind == WorkflowStepKind::Check)
            .collect::<Vec<_>>();
        for step in &checks {
            if confirm_cmd::effective_workflow_risk(&step.command, step.risk)
                > WorkflowRisk::ReadOnly
            {
                bail!("workflow check '{}' is not read-only", step.id);
            }
        }
        for step in checks {
            let command = execute_cmd::with_cwd_capture(shell, &step.command);
            let output =
                execute_cmd::run_command_capture_controlled(shell, &command, abort_signal.clone())
                    .await?;
            let result = workflow_cmd::step_result_from_output(&step.id, output);
            let stop = result.status == workflow_cmd::StepStatus::Cancelled
                || (result.status == workflow_cmd::StepStatus::Failed
                    && matches!(
                        step.on_failure,
                        WorkflowFailurePolicy::Stop | WorkflowFailurePolicy::Repair
                    ));
            check_results.push(result);
            if stop {
                break;
            }
        }

        let mut prepared = workflow_cmd::prepare_workflow(plan.clone(), &check_results)?;
        if !workflow_cmd::confirm_workflow(&prepared)? {
            return Ok((
                workflow_cmd::WorkflowStatus::Cancelled,
                "confirmation_declined",
                false,
            ));
        }
        let execution =
            workflow_cmd::execute_prepared_workflow(shell, &mut prepared, abort_signal).await;
        check_results = prepared.results.clone();
        let status = execution?;
        let interrupted = check_results
            .iter()
            .any(|result| result.status == workflow_cmd::StepStatus::Cancelled);
        let pending_termination = match status {
            workflow_cmd::WorkflowStatus::Completed => "not_run",
            workflow_cmd::WorkflowStatus::Failed => "blocked_by_failure",
            workflow_cmd::WorkflowStatus::Cancelled => "cancelled",
        };
        Ok((status, pending_termination, interrupted))
    }
    .await;

    let terminal = classify_workflow_terminal(&run_result);
    let save_result = append_workflow_terminal_note(
        &mut |note| config.write().append_session_note(note),
        &request,
        plan,
        check_results,
        repair_attempts,
        terminal.status,
        terminal.pending_termination,
    );
    match (run_result, save_result) {
        (Err(error), Err(save_error)) => {
            return Err(error.context(format!("failed to save workflow result: {save_error:#}")))
        }
        (Err(error), Ok(())) if terminal.exit_code.is_none() => return Err(error),
        (Err(_), Ok(())) => {}
        (Ok(_), Err(save_error)) => return Err(save_error),
        (Ok(_), Ok(())) => {}
    }
    if let Some(code) = terminal.exit_code {
        process::exit(code);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkflowTerminal {
    status: workflow_cmd::WorkflowStatus,
    pending_termination: &'static str,
    exit_code: Option<i32>,
}

fn classify_workflow_terminal(
    run_result: &Result<(workflow_cmd::WorkflowStatus, &'static str, bool)>,
) -> WorkflowTerminal {
    match run_result {
        Ok((status, pending_termination, interrupted)) => WorkflowTerminal {
            status: *status,
            pending_termination,
            exit_code: interrupted.then_some(130),
        },
        Err(error) if error.to_string() == "Interrupted" => WorkflowTerminal {
            status: workflow_cmd::WorkflowStatus::Cancelled,
            pending_termination: "cancelled",
            exit_code: Some(130),
        },
        Err(_) => WorkflowTerminal {
            status: workflow_cmd::WorkflowStatus::Failed,
            pending_termination: "not_run",
            exit_code: None,
        },
    }
}

fn append_workflow_terminal_note(
    append_note: &mut dyn FnMut(String) -> Result<()>,
    request: &str,
    plan: WorkflowPlan,
    results: Vec<workflow_cmd::StepResult>,
    repair_attempts: u8,
    status: workflow_cmd::WorkflowStatus,
    pending_termination: &str,
) -> Result<()> {
    let record = workflow_cmd::WorkflowRecord::from_partial(
        request.to_string(),
        plan,
        results,
        repair_attempts,
        status,
        pending_termination,
    );
    append_note(result_cmd::build_workflow_session_note(&record))
}

#[async_recursion::async_recursion]
async fn shell_execute(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    abort_signal: AbortSignal,
    retry_budget: RetryBudget,
    cache_task: Option<String>,
    repair_attempts: u8,
) -> Result<()> {
    let client = input.create_client()?;
    config.write().before_chat_completion(&input)?;
    let (raw, _) = call_chat_completions_raw_controlled(
        &input,
        client.as_ref(),
        abort_signal.clone(),
        &retry_budget,
        ProgressStage::new("正在生成命令", "Generating command"),
    )
    .await?;
    if config.read().dry_run {
        config.read().print_markdown(&raw)?;
        return Ok(());
    }
    let generated =
        parse_generated_command(&raw).context(localized("无效命令计划", "Invalid command plan"))?;
    let ask_summary = config.read().ask_summary;
    handle_generated_command(
        config,
        shell,
        input,
        abort_signal,
        ShellExecutionOptions {
            eval_str: generated.command,
            preflight: generated.preflight,
            cache_task,
            record_assistant_message: true,
            repair_attempts,
            from_cache: false,
            ask_summary,
        },
    )
    .await
}

#[derive(Debug, Clone)]
struct ShellExecutionOptions {
    eval_str: String,
    preflight: Vec<PreflightCheck>,
    cache_task: Option<String>,
    record_assistant_message: bool,
    repair_attempts: u8,
    from_cache: bool,
    ask_summary: bool,
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
        preflight,
        cache_task,
        record_assistant_message,
        repair_attempts,
        from_cache,
        ask_summary,
    } = options;
    let eval_str = eval_str.trim().to_string();

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
    if *IS_STDOUT_TERMINAL {
        let cwd = env::current_dir().context("Unable to read current directory")?;
        let report = preflight_cmd::run_checks(&preflight, &cwd);
        println!("{}", preflight_cmd::format_report(&report));
        if !report.passed() {
            let input_text = input.text();
            let task = cache_task.as_deref().unwrap_or(&input_text);
            let note = result_cmd::build_preflight_session_note(task, &report);
            config.write().append_session_note(note)?;
            return Ok(());
        }
        let client = input.create_client()?;
        let risk = confirm_cmd::classify_command_risk(&eval_str);
        loop {
            match confirm_cmd::confirm_command(&eval_str, &risk, from_cache)? {
                confirm_cmd::ConfirmationAction::Execute => {
                    let eval_command = execute_cmd::with_cwd_capture(shell, &eval_str);
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_command]);
                    let before = if risk.captures_git_changes() {
                        change_report_cmd::GitSnapshot::capture(&cwd)
                    } else {
                        None
                    };
                    let output = execute_cmd::run_command_capture_controlled(
                        shell,
                        &eval_command,
                        abort_signal.clone(),
                    )
                    .await?;
                    if let Some(before) = before {
                        if let Some(after) = change_report_cmd::GitSnapshot::capture(&cwd) {
                            let changes = before.changes_since(&after);
                            if !changes.is_empty() {
                                println!(
                                    "\n{}",
                                    change_report_cmd::format_recovery_report(&changes)
                                );
                            }
                        }
                    }
                    let (code, stdout, stderr, termination) = (
                        output.code,
                        output.stdout,
                        output.stderr,
                        output.termination,
                    );
                    if code == 0 && config.read().save_shell_history {
                        let _ = append_to_shell_history(&shell.name, &eval_str, code);
                    }
                    let summary_requested = termination == execute_cmd::CommandTermination::Exited
                        && (config.read().ai_summary
                            || (ask_summary
                                && confirm_cmd::read_action(
                                    &['y', 'n'],
                                    'n',
                                    localized(
                                        "是否生成 AI summary？[y/N] ",
                                        "Generate AI summary? [y/N] ",
                                    ),
                                )? == 'y'));
                    let summary = if summary_requested {
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
                    let session_note = result_cmd::build_execution_session_note(
                        &eval_str,
                        code,
                        termination.as_str(),
                        &stdout,
                        &stderr,
                        summary.as_deref(),
                    );
                    config.write().append_session_note(session_note)?;
                    if termination == execute_cmd::CommandTermination::Cancelled {
                        eprintln!("{}", localized("命令已取消", "Command cancelled"));
                        process::exit(130);
                    }
                    if code == 0 {
                        if let Some(task) = cache_task.as_deref() {
                            if let Err(err) = command_cache::record_success(
                                task,
                                &shell.name,
                                env::consts::OS,
                                &eval_str,
                                &preflight,
                            ) {
                                eprintln!("Command cache update failed: {err:#}");
                            }
                        }
                    }
                    if code != 0 && *IS_STDOUT_TERMINAL {
                        loop {
                            match result_cmd::prompt_failure_action(repair_attempts)? {
                                result_cmd::FailureAction::Repair => {
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
                                        RetryBudget::default(),
                                        None,
                                        repair_attempts + 1,
                                    )
                                    .await;
                                }
                                result_cmd::FailureAction::Explain => {
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
                                result_cmd::FailureAction::Copy => {
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
                confirm_cmd::ConfirmationAction::Regenerate => {
                    return request_and_route_execution_plan(
                        config,
                        shell,
                        input,
                        abort_signal.clone(),
                        RetryBudget::default(),
                        None,
                    )
                    .await;
                }
                confirm_cmd::ConfirmationAction::Revise => {
                    let revision =
                        Text::new(localized("输入修改要求:", "Enter revision:")).prompt()?;
                    let text = format!("{}\n{revision}", input.text());
                    input.set_text(text);
                    return shell_execute(
                        config,
                        shell,
                        input,
                        abort_signal.clone(),
                        RetryBudget::default(),
                        None,
                        repair_attempts,
                    )
                    .await;
                }
                confirm_cmd::ConfirmationAction::Describe => {
                    let role = config.read().retrieve_role(EXPLAIN_SHELL_ROLE)?;
                    let input = Input::from_str(config, &eval_str, Some(role));
                    let retry_budget = RetryBudget::default();
                    if input.stream() {
                        call_chat_completions_streaming_controlled(
                            &input,
                            client.as_ref(),
                            abort_signal.clone(),
                            &retry_budget,
                            ProgressStage::new("正在解释命令", "Explaining command"),
                        )
                        .await?;
                    } else {
                        call_chat_completions_controlled(
                            &input,
                            true,
                            false,
                            client.as_ref(),
                            abort_signal.clone(),
                            &retry_budget,
                            ProgressStage::new("正在解释命令", "Explaining command"),
                        )
                        .await?;
                    }
                    println!();
                    continue;
                }
                confirm_cmd::ConfirmationAction::Copy => {
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

    fn workflow_save_fixture() -> WorkflowPlan {
        plan_cmd::parse_execution_plan(
            r#"{
              "mode":"workflow","command":"","query":"","problem":"","preflight":[],
              "summary":"Save terminal workflow",
              "steps":[
                {"id":"check","kind":"check","command":"true","risk":"read_only","on_failure":"continue"},
                {"id":"write","kind":"action","command":"touch /tmp/aicmd-save","risk":"changes_files","on_failure":"stop"},
                {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"repair"}
              ]
            }"#,
        )
        .unwrap()
        .workflow()
        .unwrap()
    }

    #[test]
    fn early_workflow_failure_appends_complete_terminal_note() {
        let mut saved = Vec::new();
        append_workflow_terminal_note(
            &mut |note| {
                saved.push(note);
                Ok(())
            },
            "save workflow",
            workflow_save_fixture(),
            Vec::new(),
            0,
            workflow_cmd::WorkflowStatus::Failed,
            "not_run",
        )
        .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].matches("Step:").count(), 3);
        assert!(saved[0].contains("Workflow status: failed"));
    }

    #[test]
    fn cancelled_workflow_appends_partial_output_and_later_steps() {
        let mut saved = Vec::new();
        append_workflow_terminal_note(
            &mut |note| {
                saved.push(note);
                Ok(())
            },
            "save workflow",
            workflow_save_fixture(),
            vec![workflow_cmd::StepResult {
                step_id: "check".to_string(),
                status: workflow_cmd::StepStatus::Cancelled,
                exit_code: 130,
                termination: "cancelled".to_string(),
                stdout: "partial output".to_string(),
                stderr: String::new(),
            }],
            0,
            workflow_cmd::WorkflowStatus::Cancelled,
            "cancelled",
        )
        .unwrap();

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].matches("Step:").count(), 3);
        assert!(saved[0].contains("partial output"));
        assert!(saved[0].contains("Termination: cancelled"));
    }

    #[test]
    fn confirmation_interruption_is_saved_as_cancelled_with_exit_130() {
        let run_result: Result<(workflow_cmd::WorkflowStatus, &'static str, bool)> =
            Err(anyhow::anyhow!("Interrupted"));
        let terminal = classify_workflow_terminal(&run_result);
        let mut saved = Vec::new();
        append_workflow_terminal_note(
            &mut |note| {
                saved.push(note);
                Ok(())
            },
            "save workflow",
            workflow_save_fixture(),
            Vec::new(),
            0,
            terminal.status,
            terminal.pending_termination,
        )
        .unwrap();

        assert_eq!(terminal.status, workflow_cmd::WorkflowStatus::Cancelled);
        assert_eq!(terminal.exit_code, Some(130));
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].matches("Step:").count(), 3);
        assert!(saved[0].contains("Workflow status: cancelled"));
        assert!(saved[0].contains("Termination: cancelled"));
    }

    #[test]
    fn non_interruption_confirmation_error_remains_failed() {
        let run_result: Result<(workflow_cmd::WorkflowStatus, &'static str, bool)> =
            Err(anyhow::anyhow!("confirmation failed: Interrupted"));
        let terminal = classify_workflow_terminal(&run_result);

        assert_eq!(terminal.status, workflow_cmd::WorkflowStatus::Failed);
        assert_eq!(terminal.pending_termination, "not_run");
        assert_eq!(terminal.exit_code, None);
    }

    #[test]
    fn git_change_capture_is_limited_to_modifying_risk_levels() {
        assert!(!confirm_cmd::CommandRiskLevel::ReadOnly.captures_git_changes());
        assert!(confirm_cmd::CommandRiskLevel::ChangesSystem.captures_git_changes());
        assert!(confirm_cmd::CommandRiskLevel::Destructive.captures_git_changes());
    }

    #[test]
    fn run_in_session_intent_sets_named_session_and_task() {
        let mut cli = Cli::try_parse_from(["aicmd"]).unwrap();
        let text = translate_session_intent(
            &mut cli,
            Some(&NaturalIntent::RunInSession {
                name: "dev".to_string(),
                task: "continue this task".to_string(),
            }),
        );

        assert_eq!(cli.session, Some(Some("dev".to_string())));
        assert_eq!(text, Some(Some("continue this task".to_string())));
    }

    #[test]
    fn continue_last_failure_uses_daily_session_and_normal_planner_text() {
        let mut cli = Cli::try_parse_from([
            "aicmd", "continue", "fixing", "the", "last", "failed", "task",
        ])
        .unwrap();
        apply_intent_cli_overrides(&mut cli, Some(&NaturalIntent::ContinueLastFailure));
        let text = translate_session_intent(&mut cli, Some(&NaturalIntent::ContinueLastFailure));

        assert!(cli.no_cache);
        assert_eq!(cli.session, Some(Some(default_session_name())));
        assert_eq!(
            text,
            Some(Some("continue fixing the last failed task".to_string()))
        );
    }

    #[test]
    fn continue_last_failure_disables_cache_before_explicit_session_precedence() {
        let mut cli = Cli::try_parse_from([
            "aicmd",
            "-s",
            "cmd-20260712",
            "continue",
            "fixing",
            "the",
            "last",
            "failed",
            "task",
        ])
        .unwrap();
        let intent = NaturalIntent::ContinueLastFailure;

        apply_intent_cli_overrides(&mut cli, Some(&intent));

        assert!(cli.no_cache);
        assert!(!should_run_session_intent(&cli, Some(&intent)));
        assert_eq!(translate_session_intent(&mut cli, Some(&intent)), None);
        assert_eq!(cli.session, Some(Some("cmd-20260712".to_string())));
    }

    #[test]
    fn explicit_session_flag_keeps_natural_session_text_for_planner() {
        let mut cli = Cli::try_parse_from(["aicmd", "-s", "dev", "clear current session"]).unwrap();
        let intent = NaturalIntent::ClearSession { name: None };

        assert!(!should_run_session_intent(&cli, Some(&intent)));
        assert_eq!(translate_session_intent(&mut cli, Some(&intent)), None);
        assert_eq!(cli.session, Some(Some("dev".to_string())));
        assert!(!cli.empty_session);
        assert_eq!(cli.text_args().join(" "), "clear current session");
    }

    #[test]
    fn every_explicit_session_control_suppresses_natural_session_intents() {
        let cases = [
            (
                Cli::try_parse_from(["aicmd", "-s", "dev", "show current session"]).unwrap(),
                NaturalIntent::CurrentSession,
            ),
            (
                Cli::try_parse_from(["aicmd", "--empty-session", "clear session temp"]).unwrap(),
                NaturalIntent::ClearSession {
                    name: Some("temp".to_string()),
                },
            ),
            (
                Cli::try_parse_from(["aicmd", "--list-sessions", "list sessions"]).unwrap(),
                NaturalIntent::ListSessions,
            ),
        ];

        for (cli, intent) in cases {
            assert!(!should_run_session_intent(&cli, Some(&intent)));
        }
    }

    #[test]
    fn pre_config_session_intents_are_handled() {
        assert_eq!(
            run_pre_config_intent(Some(&NaturalIntent::CurrentSession)).unwrap(),
            Some(0)
        );
        assert_eq!(
            run_pre_config_intent(Some(&NaturalIntent::ListSessions)).unwrap(),
            Some(0)
        );
    }

    #[test]
    fn plan_request_uses_cache_only_for_an_eligible_cache_hit() {
        assert_eq!(
            plan_request_decision(true, true),
            PlanRequestDecision::UseCachedCommand
        );
        assert_eq!(
            plan_request_decision(true, false),
            PlanRequestDecision::RequestPlan
        );
        assert_eq!(
            plan_request_decision(false, true),
            PlanRequestDecision::RequestPlan
        );
        assert_eq!(
            plan_request_decision(false, false),
            PlanRequestDecision::RequestPlan
        );
    }

    #[test]
    fn command_routes_select_the_command_generation_role() {
        assert_eq!(
            command_role_for_route(RouteKind::Command),
            Some(SHELL_COMMAND_ROLE)
        );
        assert_eq!(
            command_role_for_route(RouteKind::Diagnose),
            Some(SHELL_COMMAND_ROLE)
        );
        assert_eq!(command_role_for_route(RouteKind::Search), None);
        assert_eq!(command_role_for_route(RouteKind::Workflow), None);
    }

    #[test]
    fn summary_choice_is_only_shown_when_not_preselected() {
        assert!(should_ask_for_summary(false, false, false));
        assert!(!should_ask_for_summary(true, false, false));
        assert!(!should_ask_for_summary(false, true, false));
        assert!(!should_ask_for_summary(false, false, true));
    }

    #[test]
    fn truncate_for_session_marks_truncated_content() {
        let value = result_cmd::truncate_for_session("abcdef", 3);
        assert_eq!(value, "abc\n[truncated / 已截断]");
    }

    #[test]
    fn build_execution_session_note_includes_execution_fields() {
        let note = result_cmd::build_execution_session_note(
            "printf hello",
            0,
            "exited",
            "hello",
            "",
            Some("printed hello"),
        );
        assert!(note.contains("Command execution result:"));
        assert!(note.contains("Command:\nprintf hello"));
        assert!(note.contains("Exit code: 0"));
        assert!(note.contains("STDOUT:\nhello"));
        assert!(note.contains("STDERR:\n(empty)"));
        assert!(note.contains("AI summary:\nprinted hello"));
    }
}
