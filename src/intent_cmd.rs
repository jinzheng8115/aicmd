use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NaturalIntent {
    SaveLastSearch { name: Option<String> },
    DoFromLastSearch { task: String },
    ShowRecentContext { limit: usize },
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
        assert_eq!(parse_text("查询最近 5 条日志"), None);
    }

    #[test]
    fn rejects_incomplete_or_invalid_intents() {
        assert!(parse(&["用刚才的搜索结果".to_string()]).is_err());
        assert!(parse(&["查看最近 0 条消息".to_string()]).is_err());
        assert!(parse(&["保存刚才的搜索结果为".to_string()]).is_err());
    }
}
