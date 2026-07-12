use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NaturalIntent {
    SaveLastSearch { name: Option<String> },
    DoFromLastSearch { task: String },
    ShowRecentContext { limit: usize },
    CurrentSession,
    ListSessions,
    ShowSessionRecent { name: String, limit: usize },
    ClearSession { name: Option<String> },
    RunInSession { name: String, task: String },
}

pub fn parse(args: &[String]) -> Result<Option<NaturalIntent>> {
    let text = args.join(" ");
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }

    for prefix in [
        "保存刚才的搜索结果为",
        "保存最近的搜索结果为",
        "保存刚才的搜索结果，命名为",
        "保存最近的搜索结果，命名为",
        "save the last search as ",
        "save the last search result as ",
    ] {
        if let Some(name) = strip_prefix_ignore_ascii_case(text, prefix) {
            let name = name.trim();
            if name.is_empty() {
                bail!("搜索记录名称不能为空");
            }
            return Ok(Some(NaturalIntent::SaveLastSearch {
                name: Some(name.to_string()),
            }));
        }
    }
    if matches_ignore_ascii_case(
        text,
        &[
            "保存刚才的搜索结果",
            "保存最近的搜索结果",
            "save the last search",
            "save the last search result",
        ],
    ) {
        return Ok(Some(NaturalIntent::SaveLastSearch { name: None }));
    }

    for prefix in [
        "用刚才的搜索结果",
        "使用刚才的搜索结果",
        "根据刚才的搜索结果",
        "用最近的搜索结果",
        "use the last search result to ",
        "use the last search to ",
    ] {
        if let Some(task) = strip_prefix_ignore_ascii_case(text, prefix) {
            let task = task.trim();
            if task.is_empty() {
                bail!("请说明要基于搜索结果执行什么任务");
            }
            return Ok(Some(NaturalIntent::DoFromLastSearch {
                task: task.to_string(),
            }));
        }
    }

    if let Some(limit) = parse_recent_context_limit(text)? {
        return Ok(Some(NaturalIntent::ShowRecentContext { limit }));
    }
    if matches_ignore_ascii_case(text, &["查看当前会话", "show current session"]) {
        return Ok(Some(NaturalIntent::CurrentSession));
    }
    if matches_ignore_ascii_case(text, &["列出所有会话", "列出会话", "list sessions"]) {
        return Ok(Some(NaturalIntent::ListSessions));
    }
    if matches_ignore_ascii_case(text, &["清空当前会话", "clear current session"]) {
        return Ok(Some(NaturalIntent::ClearSession { name: None }));
    }
    if let Some(intent) = parse_session_intent(text)? {
        return Ok(Some(intent));
    }
    Ok(None)
}

fn parse_session_intent(text: &str) -> Result<Option<NaturalIntent>> {
    if let Some(rest) = text.strip_prefix("查看 ") {
        if let Some((name, rest)) = rest.split_once(" 最近 ") {
            for suffix in [" 条对话", " 条消息", " 条上下文"] {
                if let Some(value) = rest.strip_suffix(suffix) {
                    if name.trim().is_empty() {
                        bail!("会话名称不能为空");
                    }
                    return Ok(Some(NaturalIntent::ShowSessionRecent {
                        name: name.trim().to_string(),
                        limit: parse_limit(value.trim())?,
                    }));
                }
            }
        }
    }

    if let Some(rest) = strip_prefix_ignore_ascii_case(text, "show last ") {
        let lower = rest.to_ascii_lowercase();
        if let Some(index) = lower.find(" messages in session ") {
            let limit = parse_limit(rest[..index].trim())?;
            let name = rest[index + " messages in session ".len()..].trim();
            if name.is_empty() || name.split_whitespace().count() != 1 {
                bail!("会话名称必须是单个非空词");
            }
            return Ok(Some(NaturalIntent::ShowSessionRecent {
                name: name.to_string(),
                limit,
            }));
        }
    }

    if let Some(name) = text.strip_prefix("清空 ") {
        if let Some(name) = name.strip_suffix(" 会话") {
            if name.trim().is_empty() {
                bail!("会话名称不能为空");
            }
            return Ok(Some(NaturalIntent::ClearSession {
                name: Some(name.trim().to_string()),
            }));
        }
        if name == "会话" {
            bail!("会话名称不能为空");
        }
    }
    if let Some(name) = strip_prefix_ignore_ascii_case(text, "clear session ") {
        if name.trim().is_empty() {
            bail!("会话名称不能为空");
        }
        return Ok(Some(NaturalIntent::ClearSession {
            name: Some(name.trim().to_string()),
        }));
    }

    if let Some(rest) = text.strip_prefix("在 ") {
        if let Some((name, task)) = rest.split_once(" 会话中") {
            if name.trim().is_empty() || task.trim().is_empty() {
                bail!("会话名称和任务不能为空");
            }
            return Ok(Some(NaturalIntent::RunInSession {
                name: name.trim().to_string(),
                task: task.trim().to_string(),
            }));
        }
    }
    if let Some(rest) = strip_prefix_ignore_ascii_case(text, "in session ") {
        let mut parts = rest.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or_default();
        let task = parts.next().unwrap_or_default().trim();
        if name.is_empty() || task.is_empty() {
            bail!("会话名称和任务不能为空");
        }
        return Ok(Some(NaturalIntent::RunInSession {
            name: name.to_string(),
            task: task.to_string(),
        }));
    }

    Ok(None)
}

fn parse_recent_context_limit(text: &str) -> Result<Option<usize>> {
    for (prefix, suffix) in [
        ("查看最近", "条对话"),
        ("查看最近", "条上下文"),
        ("查看最近", "条消息"),
    ] {
        if let Some(value) = text
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(suffix))
        {
            return parse_limit(value.trim()).map(Some);
        }
    }

    let lower = text.to_ascii_lowercase();
    for (prefix, suffix) in [
        ("show last ", " context messages"),
        ("show last ", " messages"),
        ("show recent ", " messages"),
    ] {
        if let Some(value) = lower
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(suffix))
        {
            return parse_limit(value.trim()).map(Some);
        }
    }
    Ok(None)
}

fn parse_limit(value: &str) -> Result<usize> {
    let limit = value
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("最近消息数量必须是正整数"))?;
    if limit == 0 {
        bail!("最近消息数量必须大于 0");
    }
    Ok(limit)
}

fn strip_prefix_ignore_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if text
        .get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
    {
        Some(&text[prefix.len()..])
    } else {
        None
    }
}

fn matches_ignore_ascii_case(text: &str, values: &[&str]) -> bool {
    values.iter().any(|value| text.eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_text(text: &str) -> Option<NaturalIntent> {
        parse(&[text.to_string()]).unwrap()
    }

    #[test]
    fn parses_supported_intents_without_matching_normal_tasks() {
        assert_eq!(
            parse_text("保存刚才的搜索结果为 docker-install"),
            Some(NaturalIntent::SaveLastSearch {
                name: Some("docker-install".to_string())
            })
        );
        assert_eq!(
            parse_text("用刚才的搜索结果安装 Docker"),
            Some(NaturalIntent::DoFromLastSearch {
                task: "安装 Docker".to_string()
            })
        );
        assert_eq!(
            parse_text("查看最近 5 条上下文"),
            Some(NaturalIntent::ShowRecentContext { limit: 5 })
        );
        assert_eq!(
            parse_text("show last 3 context messages"),
            Some(NaturalIntent::ShowRecentContext { limit: 3 })
        );
        assert_eq!(
            parse_text("查看当前会话"),
            Some(NaturalIntent::CurrentSession)
        );
        assert_eq!(
            parse_text("show current session"),
            Some(NaturalIntent::CurrentSession)
        );
        assert_eq!(
            parse_text("列出所有会话"),
            Some(NaturalIntent::ListSessions)
        );
        assert_eq!(
            parse_text("list sessions"),
            Some(NaturalIntent::ListSessions)
        );
        assert_eq!(
            parse_text("查看 dev 最近 5 条对话"),
            Some(NaturalIntent::ShowSessionRecent {
                name: "dev".to_string(),
                limit: 5,
            })
        );
        assert_eq!(
            parse_text("show last 3 messages in session dev"),
            Some(NaturalIntent::ShowSessionRecent {
                name: "dev".to_string(),
                limit: 3,
            })
        );
        assert_eq!(
            parse_text("清空当前会话"),
            Some(NaturalIntent::ClearSession { name: None })
        );
        assert_eq!(
            parse_text("clear session dev"),
            Some(NaturalIntent::ClearSession {
                name: Some("dev".to_string()),
            })
        );
        assert_eq!(
            parse_text("在 dev 会话中继续处理这个问题"),
            Some(NaturalIntent::RunInSession {
                name: "dev".to_string(),
                task: "继续处理这个问题".to_string(),
            })
        );
        assert_eq!(
            parse_text("in session dev continue with this task"),
            Some(NaturalIntent::RunInSession {
                name: "dev".to_string(),
                task: "continue with this task".to_string(),
            })
        );
        assert_eq!(parse_text("显示当前目录中的 session 文件"), None);
        assert_eq!(parse_text("查询最近 5 条日志"), None);
    }

    #[test]
    fn rejects_incomplete_or_invalid_intents() {
        assert!(parse(&["用刚才的搜索结果".to_string()]).is_err());
        assert!(parse(&["查看最近 0 条消息".to_string()]).is_err());
        assert!(parse(&["保存刚才的搜索结果为".to_string()]).is_err());
        assert!(parse(&["查看 dev 最近 0 条对话".to_string()]).is_err());
        assert!(parse(&["清空 会话".to_string()]).is_err());
        assert!(parse(&["在 dev 会话中".to_string()]).is_err());
        assert!(parse(&["in session dev".to_string()]).is_err());
    }
}
