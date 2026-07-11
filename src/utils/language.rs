use std::fs;

use crate::config::Config;

pub fn is_chinese() -> bool {
    let path = Config::config_file();
    let Ok(content) = fs::read_to_string(path) else {
        return true;
    };
    !content.lines().any(|line| {
        let value = line.trim().strip_prefix("language:").map(str::trim);
        matches!(value, Some("en" | "english"))
    })
}

pub fn localized<'a>(zh: &'a str, en: &'a str) -> &'a str {
    if is_chinese() {
        zh
    } else {
        en
    }
}

#[cfg(test)]
mod tests {
    use super::localized;

    #[test]
    fn localized_returns_one_language() {
        let value = localized("中文", "English");
        assert!(value == "中文" || value == "English");
        assert!(!value.contains(" / "));
    }
}
