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
    /// Unsupported in AICmd focused command workflow
    #[clap(long, hide = true)]
    pub prompt: Option<String>,
    /// Unsupported in AICmd focused command workflow
    #[clap(short, long, hide = true)]
    pub role: Option<String>,
    /// Start or join a session
    #[clap(short = 's', long)]
    pub session: Option<Option<String>>,
    /// Ensure the session is empty
    #[clap(long)]
    pub empty_session: bool,
    /// Unsupported in AICmd focused command workflow
    #[clap(short = 'a', long, hide = true)]
    pub agent: Option<String>,
    /// Set agent variables
    #[clap(long, value_names = ["NAME", "VALUE"], num_args = 2, hide = true)]
    pub agent_variable: Vec<String>,
    /// Unsupported in AICmd focused command workflow
    #[clap(long, hide = true)]
    pub rag: Option<String>,
    /// Rebuild the RAG to sync document changes
    #[clap(long, hide = true)]
    pub rebuild_rag: bool,
    /// Unsupported in AICmd focused command workflow
    #[clap(long = "macro", value_name = "MACRO", hide = true)]
    pub macro_name: Option<String>,
    /// Unsupported in AICmd focused command workflow
    #[clap(long, value_name = "ADDRESS", hide = true)]
    pub serve: Option<Option<String>>,
    /// Compatibility no-op: natural-language command execution is the default
    #[clap(short = 'e', long, hide = true)]
    pub execute: bool,
    /// Unsupported in AICmd focused command workflow
    #[clap(short = 'c', long, hide = true)]
    pub code: bool,
    /// Include files, directories, or URLs
    #[clap(short = 'f', long, value_name = "FILE")]
    pub file: Vec<String>,
    /// Turn off stream mode
    #[clap(short = 'S', long)]
    pub no_stream: bool,
    /// Display the message without sending it
    #[clap(long)]
    pub dry_run: bool,
    /// Display information
    #[clap(long)]
    pub info: bool,
    /// List all available chat models
    #[clap(long)]
    pub list_models: bool,
    /// Unsupported in AICmd focused command workflow
    #[clap(long, hide = true)]
    pub list_roles: bool,
    /// List all sessions
    #[clap(long)]
    pub list_sessions: bool,
    /// List all agents
    #[clap(long, hide = true)]
    pub list_agents: bool,
    /// List all RAGs
    #[clap(long, hide = true)]
    pub list_rags: bool,
    /// List all macros
    #[clap(long, hide = true)]
    pub list_macros: bool,
    /// Input text
    #[clap(trailing_var_arg = true)]
    text: Vec<String>,
}

impl Cli {
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
                Ok(Some(format!("{text}
{stdin_text}")))
            }
        }
    }
}
