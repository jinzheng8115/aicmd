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
use crate::client::{call_chat_completions, call_chat_completions_streaming, list_models, ModelType};
use crate::config::{
    ensure_parent_exists, load_env_file, Config, GlobalConfig, Input, CODE_ROLE,
    EXPLAIN_SHELL_ROLE, SHELL_ROLE,
};
use crate::render::render_error;
use crate::utils::*;

use anyhow::{bail, Result};
use clap::{CommandFactory, Parser};
use inquire::Text;
use parking_lot::RwLock;
use simplelog::{format_description, ConfigBuilder, LevelFilter, SimpleLogger, WriteLogger};
use std::{env, process, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    load_env_file()?;
    let cli = Cli::parse();
    let text = cli.text()?;
    let info_flag = cli.info
        || cli.list_models
        || cli.list_roles
        || cli.list_agents
        || cli.list_rags
        || cli.list_macros
        || cli.list_sessions;
    setup_logger()?;
    let config = Arc::new(RwLock::new(Config::init(info_flag).await?));
    if let Err(err) = run(config, cli, text).await {
        render_error(err);
        std::process::exit(1);
    }
    Ok(())
}

async fn run(config: GlobalConfig, cli: Cli, text: Option<String>) -> Result<()> {
    let abort_signal = create_abort_signal();

    if cli.list_models {
        for model in list_models(&config.read(), ModelType::Chat) {
            println!("{}", model.id());
        }
        return Ok(());
    }
    if cli.list_roles {
        let roles = Config::list_roles(true).join("\n");
        println!("{roles}");
        return Ok(());
    }
    if cli.dry_run {
        config.write().dry_run = true;
    }

    if cli.agent.is_some()
        || !cli.agent_variable.is_empty()
        || cli.rag.is_some()
        || cli.rebuild_rag
        || cli.macro_name.is_some()
        || cli.serve.is_some()
        || cli.list_agents
        || cli.list_rags
        || cli.list_macros
    {
        bail!("This AICmd build focuses on natural-language terminal commands. Agents, RAG, macros, and server mode are intentionally hidden.");
    }

    if let Some(prompt) = &cli.prompt {
        config.write().use_prompt(prompt)?;
    } else if let Some(name) = &cli.role {
        config.write().use_role(name)?;
    } else if cli.code {
        config.write().use_role(CODE_ROLE)?;
    } else {
        config.write().use_role(SHELL_ROLE)?;
    }
    let default_session;
    let session_name = if let Some(session) = &cli.session {
        session.as_ref().map(|v| v.as_str())
    } else {
        let beijing = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid timezone"));
        default_session = format!("cmd-{}", beijing.format("%Y%m%d"));
        Some(default_session.as_str())
    };
    config.write().use_session(session_name)?;
    if cli.list_sessions {
        let sessions = config.read().list_sessions().join("\n");
        println!("{sessions}");
        return Ok(());
    }
    if let Some(model_id) = &cli.model {
        config.write().set_model(model_id)?;
    }
    if cli.no_stream {
        config.write().stream = false;
    }
    if cli.empty_session {
        config.write().empty_session()?;
    }
    if cli.info {
        let info = config.read().info()?;
        println!("{info}");
        return Ok(());
    }
    if text.is_none() && cli.file.is_empty() {
        Cli::command().print_help()?;
        println!();
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
        let options = ["execute", "revise", "describe", "copy", "quit"];
        let command = color_text(eval_str.trim(), nu_ansi_term::Color::Rgb(255, 165, 0));
        let first_letter_color = nu_ansi_term::Color::Cyan;
        let prompt_text = options
            .iter()
            .map(|v| format!("{}{}", color_text(&v[0..1], first_letter_color), &v[1..]))
            .collect::<Vec<String>>()
            .join(&dimmed_text(" | "));
        loop {
            println!("{command}");
            let answer_char =
                read_single_key(&['e', 'r', 'd', 'c', 'q'], 'e', &format!("{prompt_text}: "))?;

            match answer_char {
                'e' => {
                    debug!("{} {:?}", shell.cmd, &[&shell.arg, &eval_str]);
                    let code = run_command(&shell.cmd, &[&shell.arg, &eval_str], None)?;
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
    let log_filter = std::env::var(get_env_name("log_filter")).unwrap_or_else(|_| crate_name.into());
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
