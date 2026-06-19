use anyhow::{Context, Result};
use clap::Parser;
use is_terminal::IsTerminal;
use std::io::{stdin, Read};

#[derive(Parser, Debug)]
#[command(author, version, about = "Run terminal commands with natural language", long_about = None)]
pub struct Cli {
    /// Select a LLM model
    #[clap(short, long)]
    pub model: Option<String>,
    /// Start or join a session
    #[clap(short = 's', long)]
    pub session: Option<Option<String>>,
    /// Ensure the session is empty
    #[clap(long)]
    pub empty_session: bool,
    /// Include files, directories, or URLs
    #[clap(short = 'f', long, value_name = "FILE")]
    pub file: Vec<String>,
    /// Display the full prompt without sending it
    #[clap(long)]
    pub dry_run: bool,
    /// Print only the generated command without interactive actions
    #[clap(long = "print")]
    pub print_command: bool,
    /// List all sessions
    #[clap(long)]
    pub list_sessions: bool,
    /// Input text
    #[clap(trailing_var_arg = true)]
    text: Vec<String>,
}

impl Cli {
    pub fn text_args(&self) -> &[String] {
        &self.text
    }

    pub fn text(&self) -> Result<Option<String>> {
        let mut stdin_text = String::new();
        if !stdin().is_terminal() {
            let _ = stdin()
                .read_to_string(&mut stdin_text)
                .context("Invalid stdin pipe")?;
        };
        if self.text.is_empty() {
            if stdin_text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(stdin_text))
            }
        } else {
            let text = self.text.join(" ");
            if stdin_text.is_empty() {
                Ok(Some(text))
            } else {
                Ok(Some(format!(
                    "{text}
{stdin_text}"
                )))
            }
        }
    }
}
