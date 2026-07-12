use crate::cli::Cli;
use crate::utils::localized;
use anyhow::{Context, Result};
use std::{
    env,
    io::{self, Write},
    process::Command,
};

pub fn is_eligible(cli: &Cli) -> bool {
    cli.text_args().is_empty()
        && cli.model.is_none()
        && cli.session.is_none()
        && !cli.empty_session
        && cli.file.is_empty()
        && !cli.dry_run
        && !cli.print_command
        && !cli.summary
        && !cli.no_summary
        && !cli.no_cache
        && !cli.list_sessions
}

pub fn should_start(cli: &Cli, stdin_is_terminal: bool, stdout_is_terminal: bool) -> bool {
    is_eligible(cli) && stdin_is_terminal && stdout_is_terminal
}

pub fn is_exit_input(input: &str) -> bool {
    matches!(input.trim(), "exit" | "quit" | ".exit")
}

pub fn child_args(session: &str, input: &str) -> Vec<String> {
    vec!["-s".to_string(), session.to_string(), input.to_string()]
}

pub fn run(session: &str) -> Result<i32> {
    let exe = env::current_exe().context("Unable to resolve current executable")?;
    println!("AICmd {}", env!("CARGO_PKG_VERSION"));
    println!("{}: {session}", localized("会话", "Session"));
    println!(
        "{}",
        localized(
            "输入任务，输入 exit 退出。",
            "Enter a task; type exit to quit."
        )
    );
    println!();

    loop {
        print!("AICmd> ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => return Ok(0),
            Ok(_) => {}
            Err(err) => return Err(err.into()),
        }

        let input = input.trim();
        if is_exit_input(input) {
            return Ok(0);
        }
        if input.is_empty() {
            continue;
        }

        Command::new(&exe)
            .args(child_args(session, input))
            .status()
            .with_context(|| format!("Unable to start {}", exe.display()))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn cli(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn recognizes_exit_inputs() {
        assert!(is_exit_input("exit"));
        assert!(is_exit_input(" quit "));
        assert!(is_exit_input(".exit"));
        assert!(!is_exit_input("exit vim"));
    }

    #[test]
    fn builds_session_child_args_without_splitting_input() {
        assert_eq!(
            child_args("cmd-20260712", "查看内存"),
            vec!["-s", "cmd-20260712", "查看内存"]
        );
    }

    #[test]
    fn no_arguments_are_prompt_eligible() {
        assert!(is_eligible(&cli(&["aicmd"])));
    }

    #[test]
    fn prompt_requires_both_terminal_streams() {
        let cli = cli(&["aicmd"]);
        assert!(should_start(&cli, true, true));
        assert!(!should_start(&cli, true, false));
        assert!(!should_start(&cli, false, true));
        assert!(!should_start(&cli, false, false));
    }

    #[test]
    fn explicit_inputs_and_options_are_not_prompt_eligible() {
        let cases = [
            cli(&["aicmd", "查看内存"]),
            cli(&["aicmd", "--dry-run"]),
            cli(&["aicmd", "--print"]),
            cli(&["aicmd", "--model", "test"]),
            cli(&["aicmd", "--session", "dev"]),
            cli(&["aicmd", "--file", "README.md"]),
            cli(&["aicmd", "--summary"]),
            cli(&["aicmd", "--no-summary"]),
            cli(&["aicmd", "--no-cache"]),
            cli(&["aicmd", "--list-sessions"]),
            cli(&["aicmd", "--empty-session"]),
        ];

        for cli in cases {
            assert!(!is_eligible(&cli));
        }
    }
}
