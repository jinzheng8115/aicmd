mod cli;
mod client;
mod config;
mod function;
mod render;
#[macro_use]
mod utils;

#[macro_use]
extern crate log;

use crate::cli::Cli;
use crate::client::{call_chat_completions, call_chat_completions_streaming};
use crate::config::{
    ensure_parent_exists, load_env_file, Config, GlobalConfig, Input, EXPLAIN_SHELL_ROLE,
    MCP_SUMMARY_ROLE, SHELL_ROLE,
};
use crate::render::render_error;
use crate::utils::*;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use inquire::Text;
use parking_lot::RwLock;
use simplelog::{format_description, ConfigBuilder, LevelFilter, SimpleLogger, WriteLogger};
use std::{
    env,
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Write},
    process,
    sync::Arc,
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

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn command_with_cwd_capture(command: &str) -> String {
    let Ok(cwd_file) = env::var("AICMD_CWD_FILE") else {
        return command.to_string();
    };
    if cwd_file.is_empty() {
        return command.to_string();
    }
    format!(
        "{{\n{command}\n}}\n__aicmd_status=$?\npwd > {}\nexit $__aicmd_status",
        shell_single_quote(&cwd_file)
    )
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
        _ => Ok(None),
    }
}

async fn run_builtin_shortcut(config: &GlobalConfig, args: &[String]) -> Result<Option<i32>> {
    let Some(cmd) = args.first().map(String::as_str) else {
        return Ok(None);
    };
    match cmd {
        "search" => {
            if args.len() < 2 {
                bail!("usage: aicmd search <query>");
            }
            let query = args[1..].join(" ");
            run_mcp_with_llm_summary(config, "search", &query).await?;
            Ok(Some(0))
        }
        "mcp" => {
            if args.len() < 2 {
                bail!("usage: aicmd mcp <command> [args...]");
            }
            let mcp_command = &args[1];
            let query = args[2..].join(" ");
            run_mcp_with_llm_summary(config, mcp_command, &query).await?;
            Ok(Some(0))
        }
        _ => Ok(None),
    }
}

async fn run_mcp_with_llm_summary(
    config: &GlobalConfig,
    mcp_command: &str,
    query: &str,
) -> Result<()> {
    let abort_signal = create_abort_signal();
    let mcp_args = if query.trim().is_empty() {
        vec![mcp_command]
    } else {
        vec![mcp_command, query]
    };
    let (success, stdout, stderr) = run_command_with_output("aicmd-mcp", &mcp_args, None)
        .with_context(|| "Unable to run aicmd-mcp")?;
    let raw_output = if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        format!(
            "{}

{}",
            stdout.trim(),
            stderr.trim()
        )
    };
    if raw_output.trim().is_empty() {
        bail!("MCP command returned no output");
    }
    let status_text = if success { "success" } else { "failed" };
    let prompt = format!(
        "MCP command: {mcp_command}
MCP status: {status_text}
用户请求：{query}

MCP 原始返回：
{raw_output}"
    );
    let role = config.read().retrieve_role(MCP_SUMMARY_ROLE)?;
    let input = Input::from_str(config, &prompt, Some(role));
    let client = input.create_client()?;
    if input.stream() {
        call_chat_completions_streaming(&input, client.as_ref(), abort_signal).await?;
    } else {
        call_chat_completions(&input, true, false, client.as_ref(), abort_signal).await?;
    }
    println!();
    Ok(())
}

async fn run(config: GlobalConfig, cli: Cli, text: Option<String>) -> Result<()> {
    let abort_signal = create_abort_signal();

    if cli.dry_run {
        config.write().dry_run = true;
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
    config.write().use_session(session_name)?;
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
    let input = create_input(&config, text, &cli.file, abort_signal.clone()).await?;
    shell_execute(&config, &SHELL, input, abort_signal.clone()).await?;
    Ok(())
}

#[async_recursion::async_recursion]
async fn shell_execute(
    config: &GlobalConfig,
    shell: &Shell,
    mut input: Input,
    abort_signal: AbortSignal,
) -> Result<()> {
    let client = input.create_client()?;
    config.write().before_chat_completion(&input)?;
    let (eval_str, _) =
        call_chat_completions(&input, false, true, client.as_ref(), abort_signal.clone()).await?;

    config.write().after_chat_completion(&input, &eval_str)?;
    if eval_str.is_empty() {
        bail!("No command generated");
    }
    if config.read().dry_run {
        config.read().print_markdown(&eval_str)?;
        return Ok(());
    }
    if *IS_STDOUT_TERMINAL {
        let options = [
            ("e", "xecute", "执行"),
            ("r", "evise", "修改"),
            ("d", "escribe", "解释"),
            ("c", "opy", "复制"),
            ("q", "uit", "退出"),
        ];
        let command = color_text(eval_str.trim(), nu_ansi_term::Color::Rgb(255, 165, 0));
        let first_letter_color = nu_ansi_term::Color::Cyan;
        let prompt_text = options
            .iter()
            .map(|(key, rest, zh)| {
                format!("{}{}({})", color_text(key, first_letter_color), rest, zh)
            })
            .collect::<Vec<String>>()
            .join(&dimmed_text(" | "));
        loop {
            println!("{command}");
            let answer_char =
                read_single_key(&['e', 'r', 'd', 'c', 'q'], 'e', &format!("{prompt_text}: "))?;

            match answer_char {
                'e' => {
                    let eval_command = command_with_cwd_capture(&eval_str);
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_command]);
                    let code = run_command(&shell.cmd, &[&shell.arg, &eval_command], None)?;
                    if code == 0 && config.read().save_shell_history {
                        let _ = append_to_shell_history(&shell.name, &eval_str, code);
                    }
                    process::exit(code);
                }
                'r' => {
                    let revision = Text::new("Enter your revision:").prompt()?;
                    let text = format!("{}\n{revision}", input.text());
                    input.set_text(text);
                    return shell_execute(config, shell, input, abort_signal.clone()).await;
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
                }
                _ => {}
            }
            break;
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
