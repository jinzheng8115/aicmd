use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    env,
    fs::{self, read_to_string},
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::client::{Message, MessageRole};

const CONFIG_DIR_ENV: &str = "AICMD_CONFIG_DIR";
const SESSIONS_DIR_ENV: &str = "AICMD_SESSIONS_DIR";
const SESSIONS_DIR_NAME: &str = "sessions";
const DEFAULT_LIMIT: usize = 20;

#[derive(Debug, Default, Deserialize)]
struct SessionFile {
    #[serde(default)]
    role_name: Option<String>,
    #[serde(default)]
    messages: Vec<Message>,
}

#[derive(Debug)]
struct SessionSummary {
    name: String,
    path: PathBuf,
    modified: Option<SystemTime>,
    messages: usize,
}

#[derive(Debug)]
struct SessionShowOptions {
    name: Option<String>,
    limit: usize,
}

pub fn run_session_command(args: &[String]) -> Result<i32> {
    match args.first().map(String::as_str) {
        None => print_current_session(),
        Some("list") | Some("ls") => list_sessions(),
        Some("show") => show_session(&args[1..]),
        Some("help") | Some("-h") | Some("--help") => {
            print_session_usage();
            Ok(0)
        }
        Some(arg) => bail!("Unknown session command: {arg}"),
    }
}

pub fn run_last_command(args: &[String]) -> Result<i32> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "-h" | "--help"))
    {
        print_last_usage();
        return Ok(0);
    }
    if !args.is_empty() {
        bail!("Unknown option for last: {}", args.join(" "));
    }
    let name = current_session_name();
    let path = session_file(&name);
    if !path.exists() {
        bail!("Session not found: {name} ({})", path.display());
    }
    let session = load_session(&path)?;
    let Some((index, message)) = session
        .messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| !message.role.is_system())
    else {
        println!("No messages in session: {name}");
        return Ok(0);
    };

    println!("Session: {name}");
    println!("File: {}", path.display());
    println!("Message: {} / {}", index + 1, session.messages.len());
    println!("Role: {}", role_label(message.role));
    println!("---");
    println!("{}", message.content.to_text().trim_end());
    Ok(0)
}

fn print_current_session() -> Result<i32> {
    let name = current_session_name();
    let path = session_file(&name);
    println!("Current session: {name}");
    println!("当前会话：{name}");
    println!("File: {}", path.display());
    if path.exists() {
        let summary = session_summary(&name, &path)?;
        println!("Messages: {}", summary.messages);
        println!("Updated: {}", format_system_time(summary.modified));
    } else {
        println!("Messages: 0");
        println!("Status: not created yet / 状态：尚未创建");
    }
    Ok(0)
}

fn list_sessions() -> Result<i32> {
    let mut sessions = collect_sessions()?;
    if sessions.is_empty() {
        println!("No sessions found. / 没有找到会话。");
        println!("Sessions dir: {}", sessions_dir().display());
        return Ok(0);
    }
    sessions.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    println!("Sessions dir: {}", sessions_dir().display());
    println!("{:<32} {:>8}  {:<19} File", "Name", "Messages", "Updated");
    for item in sessions {
        println!(
            "{:<32} {:>8}  {:<19} {}",
            item.name,
            item.messages,
            format_system_time(item.modified),
            item.path.display()
        );
    }
    Ok(0)
}

fn show_session(args: &[String]) -> Result<i32> {
    let options = parse_show_options(args)?;
    let name = options.name.unwrap_or_else(current_session_name);
    let path = session_file(&name);
    if !path.exists() {
        bail!("Session not found: {name} ({})", path.display());
    }
    let session = load_session(&path)?;
    let messages: Vec<_> = session
        .messages
        .iter()
        .enumerate()
        .filter(|(_, message)| !message.role.is_system())
        .collect();
    let total = messages.len();
    let start = total.saturating_sub(options.limit);

    println!("Session: {name}");
    println!("File: {}", path.display());
    if let Some(role_name) = session.role_name.as_deref() {
        println!("Role: {role_name}");
    }
    println!(
        "Messages: {total} (showing {})",
        total.saturating_sub(start)
    );
    println!("---");
    for (display_index, (_original_index, message)) in messages.into_iter().skip(start).enumerate()
    {
        println!("[{}] {}", display_index + 1, role_label(message.role));
        println!("{}", trim_for_terminal(&message.content.to_text(), 4000));
        println!("---");
    }
    Ok(0)
}

fn parse_show_options(args: &[String]) -> Result<SessionShowOptions> {
    let mut options = SessionShowOptions {
        name: None,
        limit: DEFAULT_LIMIT,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--limit" | "-n" => {
                let Some(value) = args.get(index + 1) else {
                    bail!("--limit requires a value");
                };
                options.limit = parse_limit(value)?;
                index += 2;
            }
            value if value.starts_with("--limit=") => {
                options.limit = parse_limit(value.trim_start_matches("--limit="))?;
                index += 1;
            }
            "help" | "-h" | "--help" => {
                print_session_usage();
                std::process::exit(0);
            }
            value if value.starts_with('-') => bail!("Unknown option for session show: {value}"),
            value => {
                if options.name.is_some() {
                    bail!("Only one session name can be provided");
                }
                options.name = Some(value.to_string());
                index += 1;
            }
        }
    }
    Ok(options)
}

fn parse_limit(value: &str) -> Result<usize> {
    let limit = value
        .parse::<usize>()
        .with_context(|| format!("Invalid limit: {value}"))?;
    if limit == 0 {
        bail!("--limit must be greater than 0");
    }
    Ok(limit)
}

fn collect_sessions() -> Result<Vec<SessionSummary>> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut sessions = vec![];
    collect_session_files(&dir, &dir, &mut sessions)?;
    Ok(sessions)
}

fn collect_session_files(
    root: &Path,
    dir: &Path,
    sessions: &mut Vec<SessionSummary>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_session_files(root, &path, sessions)?;
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("yaml") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let mut name = rel.to_string_lossy().replace('\\', "/");
        if let Some(stripped) = name.strip_suffix(".yaml") {
            name = stripped.to_string();
        }
        sessions.push(session_summary(&name, &path)?);
    }
    Ok(())
}

fn session_summary(name: &str, path: &Path) -> Result<SessionSummary> {
    let metadata = fs::metadata(path).ok();
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());
    let messages = load_session(path)
        .map(|session| session.messages.len())
        .unwrap_or(0);
    Ok(SessionSummary {
        name: name.to_string(),
        path: path.to_path_buf(),
        modified,
        messages,
    })
}

fn load_session(path: &Path) -> Result<SessionFile> {
    let content =
        read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Invalid session file: {}", path.display()))
}

fn current_session_name() -> String {
    chrono::Utc::now()
        .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).expect("valid timezone"))
        .format("cmd-%Y%m%d")
        .to_string()
}

fn session_file(name: &str) -> PathBuf {
    match name.split_once('/') {
        Some((dir, file)) => sessions_dir().join(dir).join(format!("{file}.yaml")),
        None => sessions_dir().join(format!("{name}.yaml")),
    }
}

fn sessions_dir() -> PathBuf {
    if let Ok(value) = env::var(SESSIONS_DIR_ENV) {
        return PathBuf::from(value);
    }
    config_dir().join(SESSIONS_DIR_NAME)
}

fn config_dir() -> PathBuf {
    if let Ok(value) = env::var(CONFIG_DIR_ENV) {
        return PathBuf::from(value);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".aicmd")
}

fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::Assistant => "assistant",
        MessageRole::User => "user",
        MessageRole::Tool => "tool",
    }
}

fn trim_for_terminal(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            out.push_str("\n...[truncated]");
            return out;
        }
        out.push(ch);
    }
    out
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "unknown".to_string();
    };
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn print_session_usage() {
    println!(
        r#"Usage: aicmd session
       aicmd session list
       aicmd session show [SESSION] [--limit N]

用法：aicmd session
      aicmd session list
      aicmd session show [会话名] [--limit N]

Commands / 命令:
  session             Show current default session / 显示当前默认会话
  session list        List saved sessions / 列出已保存会话
  session show        Show recent messages / 查看最近消息
"#
    );
}

fn print_last_usage() {
    println!(
        r#"Usage: aicmd last

Show the last non-system message in the current default session.

用法：aicmd last

显示当前默认会话里最后一条非 system 消息。"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_show_limit() {
        let args = vec!["dev".to_string(), "--limit".to_string(), "3".to_string()];
        let options = parse_show_options(&args).unwrap();
        assert_eq!(options.name.as_deref(), Some("dev"));
        assert_eq!(options.limit, 3);
    }

    #[test]
    fn trim_marks_truncated_content() {
        let value = trim_for_terminal("abcdef", 3);
        assert_eq!(value, "abc\n...[truncated]");
    }
}
