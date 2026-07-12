use crate::utils::{color_text, dimmed_text, localized, read_single_key};
use anyhow::Result;
use nu_ansi_term::Color;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};

pub fn confirm_high_risk(message: &str) -> Result<bool> {
    if let Ok(tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") {
        let mut reader = BufReader::new(tty.try_clone()?);
        let mut writer = tty;
        write!(writer, "{message} [y/N] ")?;
        writer.flush()?;
        let mut answer = String::new();
        reader.read_line(&mut answer)?;
        return Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"));
    }
    eprint!("{message} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

pub fn read_action(keys: &[char], default: char, prompt: &str) -> Result<char> {
    read_single_key(keys, default, prompt)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRiskLevel {
    ReadOnly,
    ChangesSystem,
    Destructive,
}

impl CommandRiskLevel {
    pub fn captures_git_changes(self) -> bool {
        matches!(self, Self::ChangesSystem | Self::Destructive)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRisk {
    level: CommandRiskLevel,
    reasons: Vec<&'static str>,
}

impl CommandRisk {
    fn label(&self) -> &'static str {
        match self.level {
            CommandRiskLevel::ReadOnly => localized("只读", "read-only"),
            CommandRiskLevel::ChangesSystem => localized("会修改系统或文件", "changes system"),
            CommandRiskLevel::Destructive => localized("可能造成破坏", "destructive"),
        }
    }

    pub fn requires_confirmation(&self) -> bool {
        matches!(self.level, CommandRiskLevel::Destructive)
    }

    pub fn captures_git_changes(&self) -> bool {
        self.level.captures_git_changes()
    }

    fn display(&self) -> String {
        if self.reasons.is_empty() {
            format!("{}: {}", localized("风险", "Risk"), self.label())
        } else {
            format!(
                "{}: {} ({})",
                localized("风险", "Risk"),
                self.label(),
                self.reasons.join(", ")
            )
        }
    }
}

pub fn classify_command_risk(command: &str) -> CommandRisk {
    let lower = command.to_lowercase();
    let mut level = CommandRiskLevel::ReadOnly;
    let mut reasons = Vec::new();
    for (pattern, reason) in [
        ("rm -rf", "recursive force delete"),
        ("rm -fr", "recursive force delete"),
        ("mkfs", "format filesystem"),
        ("dd if=", "raw disk write/copy"),
        ("diskutil erase", "erase disk"),
        ("docker system prune", "docker prune"),
        ("git reset --hard", "discard git changes"),
        ("git clean -fd", "delete untracked files"),
        ("chmod -r", "recursive permission change"),
        ("chown -r", "recursive owner change"),
        ("drop database", "drop database"),
        ("truncate table", "truncate table"),
        ("delete from", "database delete"),
    ] {
        if lower.contains(pattern) {
            level = CommandRiskLevel::Destructive;
            reasons.push(reason);
        }
    }
    if !matches!(level, CommandRiskLevel::Destructive) {
        for (pattern, reason) in [
            ("sudo ", "sudo"),
            (" >", "redirect write"),
            (">>", "append write"),
            ("tee ", "write file"),
            ("mkdir ", "create directory"),
            ("touch ", "create/update file"),
            ("mv ", "move/rename"),
            ("cp ", "copy"),
            ("rm ", "delete"),
            ("chmod ", "permission change"),
            ("chown ", "owner change"),
            ("install ", "install"),
            ("npm install", "install package"),
            ("brew install", "install package"),
            ("apt install", "install package"),
            ("apt-get install", "install package"),
            ("pip install", "install package"),
            ("curl ", "network"),
            ("wget ", "network"),
            ("docker run", "start container"),
            ("docker compose up", "start containers"),
            ("systemctl ", "service control"),
            ("launchctl ", "service control"),
        ] {
            if lower.contains(pattern) {
                level = CommandRiskLevel::ChangesSystem;
                reasons.push(reason);
            }
        }
    }
    reasons.sort_unstable();
    reasons.dedup();
    CommandRisk { level, reasons }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationAction {
    Execute,
    Revise,
    Describe,
    Copy,
    Regenerate,
    Quit,
}

pub fn confirm_command(
    command: &str,
    risk: &CommandRisk,
    from_cache: bool,
) -> Result<ConfirmationAction> {
    loop {
        println!("{}", color_text(command, Color::Rgb(255, 165, 0)));
        println!("{}", dimmed_text(&risk.display()));
        let mut answer = read_action(
            &['y', 'n', '?'],
            'y',
            localized("执行？[Y/n/?] ", "Run? [Y/n/?] "),
        )?;
        if answer == '?' {
            let mut keys = vec!['r', 'd', 'c', 'q'];
            let mut options = vec![
                format!(
                    "{}{}",
                    color_text("r", Color::Cyan),
                    localized(" 修改", "evise")
                ),
                format!(
                    "{}{}",
                    color_text("d", Color::Cyan),
                    localized(" 解释", "escribe")
                ),
                format!(
                    "{}{}",
                    color_text("c", Color::Cyan),
                    localized(" 复制", "opy")
                ),
                format!(
                    "{}{}",
                    color_text("q", Color::Cyan),
                    localized(" 退出", "uit")
                ),
            ];
            if from_cache {
                keys.insert(0, 'g');
                options.insert(
                    0,
                    format!(
                        "{}{}",
                        color_text("g", Color::Cyan),
                        localized(" 重新生成", "enerate")
                    ),
                );
            }
            answer = read_action(
                &keys,
                'q',
                &format!(
                    "{}：{}: ",
                    localized("更多", "More"),
                    options.join(&dimmed_text(" | "))
                ),
            )?;
        }
        let action = match answer {
            'y' => ConfirmationAction::Execute,
            'g' if from_cache => ConfirmationAction::Regenerate,
            'r' => ConfirmationAction::Revise,
            'd' => ConfirmationAction::Describe,
            'c' => ConfirmationAction::Copy,
            _ => ConfirmationAction::Quit,
        };
        if action == ConfirmationAction::Execute
            && risk.requires_confirmation()
            && !confirm_high_risk(localized(
                "高风险命令，确认执行？",
                "High-risk command. Continue?",
            ))?
        {
            println!("{}", localized("已取消", "cancelled"));
            continue;
        }
        return Ok(action);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_commands_require_a_second_confirmation() {
        assert!(classify_command_risk("rm -rf /tmp/aicmd-test").requires_confirmation());
        assert!(!classify_command_risk("pwd").requires_confirmation());
    }
}
