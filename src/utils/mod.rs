mod abort_signal;
mod clipboard;
mod command;
mod crypto;
mod html_to_md;
mod input;
mod loader;
mod path;
mod request;
mod spinner;
mod variables;

pub use self::abort_signal::*;
pub use self::clipboard::set_text;
pub use self::command::*;
pub use self::crypto::*;
pub use self::html_to_md::*;
pub use self::input::*;
pub use self::loader::*;
pub use self::path::*;
pub use self::request::*;
pub use self::spinner::*;
pub use self::variables::*;

use anyhow::{Context, Result};
use fancy_regex::Regex;
use is_terminal::IsTerminal;
use std::borrow::Cow;
use std::sync::LazyLock;
use std::{env, path::PathBuf, process};
use unicode_segmentation::UnicodeSegmentation;

pub static CODE_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?ms)```\w*(.*)```").unwrap());
pub static THINK_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*<think>.*?</think>(\s*|$)").unwrap());
pub static IS_STDOUT_TERMINAL: LazyLock<bool> = LazyLock::new(|| std::io::stdout().is_terminal());
pub static NO_COLOR: LazyLock<bool> = LazyLock::new(|| {
    env::var("NO_COLOR")
        .ok()
        .and_then(|v| parse_bool(&v))
        .unwrap_or_default()
        || !*IS_STDOUT_TERMINAL
});

pub fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn now_timestamp() -> i64 {
    chrono::Local::now().timestamp()
}

pub fn get_env_name(key: &str) -> String {
    format!("{}_{key}", env!("CARGO_CRATE_NAME"),).to_ascii_uppercase()
}

pub fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

pub fn estimate_token_length(text: &str) -> usize {
    let words: Vec<&str> = text.unicode_words().collect();
    let mut output: f32 = 0.0;
    for word in words {
        if word.is_ascii() {
            output += 1.3;
        } else {
            let count = word.chars().count();
            if count == 1 {
                output += 1.0
            } else {
                output += (count as f32) * 0.5;
            }
        }
    }
    output.ceil() as usize
}

pub fn strip_think_tag(text: &str) -> Cow<'_, str> {
    THINK_TAG_RE.replace_all(text, "")
}

pub fn extract_code_block(text: &str) -> &str {
    CODE_BLOCK_RE
        .captures(text)
        .ok()
        .and_then(|v| v?.get(1).map(|v| v.as_str().trim()))
        .unwrap_or(text)
}

pub fn clean_terminal_markdown(text: &str) -> String {
    let mut in_fence = false;
    let mut out = Vec::new();
    for raw_line in text.lines() {
        let mut line = raw_line.to_string();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            line = clean_markdown_line(&line);
        }
        out.push(line);
    }
    let mut value = out.join("\n");
    if text.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn clean_markdown_line(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let mut text = rest.to_string();

    let heading =
        strip_heading_marker(&text).map(|(level, stripped)| (level, stripped.to_string()));
    let heading_level = heading.as_ref().map(|(level, _)| *level);
    if let Some((_, stripped)) = heading {
        text = stripped;
    }

    text = strip_list_marker(&text).to_string();
    text = text.replace("**", "").replace("__", "");
    text = text.replace('`', "");
    let text = match heading_level {
        Some(level) => heading_text(&text, level),
        None => text,
    };
    format!("{indent}{text}")
}

fn strip_heading_marker(value: &str) -> Option<(usize, &str)> {
    let trimmed = value.trim_start_matches('#');
    let hashes = value.len() - trimmed.len();
    if (1..=6).contains(&hashes) && trimmed.starts_with(' ') {
        Some((hashes, trimmed.trim_start()))
    } else {
        None
    }
}

fn strip_list_marker(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && matches!(bytes[0], b'-' | b'*') && bytes[1] == b' ' {
        return &value[2..];
    }
    value
}

pub fn convert_option_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn pretty_error(err: &anyhow::Error) -> String {
    let mut output = vec![];
    output.push(format!("Error: {err}"));
    let causes: Vec<_> = err.chain().skip(1).collect();
    let causes_len = causes.len();
    if causes_len > 0 {
        output.push("\nCaused by:".to_string());
        if causes_len == 1 {
            output.push(format!("    {}", indent_text(causes[0], 4).trim()));
        } else {
            for (i, cause) in causes.into_iter().enumerate() {
                output.push(format!("{i:5}: {}", indent_text(cause, 7).trim()));
            }
        }
    }
    output.join("\n")
}

pub fn indent_text<T: ToString>(s: T, size: usize) -> String {
    let indent_str = " ".repeat(size);
    s.to_string()
        .split('\n')
        .map(|line| format!("{indent_str}{line}"))
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn error_text(input: &str) -> String {
    color_text(input, nu_ansi_term::Color::Red)
}

pub fn warning_text(input: &str) -> String {
    color_text(input, nu_ansi_term::Color::Yellow)
}

pub fn color_text(input: &str, color: nu_ansi_term::Color) -> String {
    if *NO_COLOR {
        return input.to_string();
    }
    nu_ansi_term::Style::new()
        .fg(color)
        .paint(input)
        .to_string()
}

pub fn heading_text(input: &str, level: usize) -> String {
    if *NO_COLOR {
        return input.to_string();
    }
    let color = if level <= 1 {
        nu_ansi_term::Color::Cyan
    } else {
        nu_ansi_term::Color::Blue
    };
    nu_ansi_term::Style::new()
        .bold()
        .fg(color)
        .paint(input)
        .to_string()
}

pub fn dimmed_text(input: &str) -> String {
    if *NO_COLOR {
        return input.to_string();
    }
    nu_ansi_term::Style::new().dimmed().paint(input).to_string()
}

pub fn temp_file(prefix: &str, suffix: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "{}-{}{prefix}{}{suffix}",
        env!("CARGO_CRATE_NAME").to_lowercase(),
        process::id(),
        uuid::Uuid::new_v4()
    ))
}

pub fn is_url(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://")
}

pub fn set_proxy(
    mut builder: reqwest::ClientBuilder,
    proxy: &str,
) -> Result<reqwest::ClientBuilder> {
    builder = builder.no_proxy();
    if !proxy.is_empty() && proxy != "-" {
        builder = builder
            .proxy(reqwest::Proxy::all(proxy).with_context(|| format!("Invalid proxy `{proxy}`"))?);
    };
    Ok(builder)
}

pub fn decode_bin<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T> {
    let (v, _) = bincode::serde::decode_from_slice(data, bincode::config::legacy())?;
    Ok(v)
}

#[cfg(test)]
mod terminal_markdown_tests {
    use super::clean_terminal_markdown;

    #[test]
    fn removes_common_markdown_markers_for_terminal_output() {
        let input = "### 安装 Docker 的步骤\n\n#### 1. 使用 Homebrew 安装 Docker\n- **打开终端**\n```bash\nbrew install docker\n```\n";
        let output = clean_terminal_markdown(input);
        assert!(!output.contains("###"));
        assert!(!output.contains("####"));
        assert!(!output.contains("**"));
        assert!(!output.contains("```"));
        assert!(output.contains("安装 Docker 的步骤"));
        assert!(output.contains("使用 Homebrew 安装 Docker"));
        assert!(output.contains("打开终端"));
        assert!(output.contains("brew install docker"));
    }
}
