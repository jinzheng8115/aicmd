use crate::utils::localized;
use std::{collections::BTreeMap, path::Path, process::Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    records: BTreeMap<String, String>,
}

impl GitSnapshot {
    pub fn capture(cwd: &Path) -> Option<Self> {
        let output = Command::new("git")
            .args(["status", "--porcelain=v1", "--untracked-files=all"])
            .current_dir(cwd)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        Some(Self::from_porcelain(&stdout))
    }

    pub fn changes_since(&self, after: &GitSnapshot) -> Vec<String> {
        after
            .records
            .iter()
            .filter(|(path, record)| self.records.get(*path) != Some(*record))
            .map(|(_, record)| record.clone())
            .collect()
    }

    fn from_porcelain(output: &str) -> Self {
        let records = output
            .lines()
            .filter_map(|record| {
                let path = record.get(3..)?;
                Some((path.to_string(), record.to_string()))
            })
            .collect();
        Self { records }
    }
}

pub fn format_recovery_report(changes: &[String]) -> String {
    let records = changes
        .iter()
        .map(|change| format!("- {change}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n{records}\n\n{}\n1. {}\n2. {}",
        localized("检测到文件变化：", "Detected file changes:"),
        localized("恢复建议：", "Recovery guidance:"),
        localized(
            "使用 git diff 查看已跟踪文件变化。",
            "Use git diff to inspect tracked file changes."
        ),
        localized(
            "确认后再手动恢复；AICmd 不会自动删除或重置文件。",
            "Recover manually after inspection; AICmd does not automatically reset or delete files."
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_new_status_records() {
        let before = GitSnapshot::from_porcelain(" M existing.txt\n");
        let after = GitSnapshot::from_porcelain(" M existing.txt\n?? new.txt\n");

        assert_eq!(before.changes_since(&after), vec!["?? new.txt"]);
    }

    #[test]
    fn reports_changed_status_for_the_same_path() {
        let before = GitSnapshot::from_porcelain(" M existing.txt\n");
        let after = GitSnapshot::from_porcelain("M  existing.txt\n");

        assert_eq!(before.changes_since(&after), vec!["M  existing.txt"]);
    }

    #[test]
    fn omits_unchanged_status_records() {
        let before = GitSnapshot::from_porcelain(" M existing.txt\n?? new.txt\n");
        let after = GitSnapshot::from_porcelain(" M existing.txt\n?? new.txt\n");

        assert!(before.changes_since(&after).is_empty());
    }

    #[test]
    fn preserves_spaces_renames_and_non_ascii_paths() {
        let before = GitSnapshot::from_porcelain("");
        let after =
            GitSnapshot::from_porcelain("?? path with spaces.txt\nR  old name.txt -> 新名称.txt\n");

        assert_eq!(
            before.changes_since(&after),
            vec!["R  old name.txt -> 新名称.txt", "?? path with spaces.txt"]
        );
    }

    #[test]
    fn recovery_report_recommends_manual_git_inspection() {
        let report = format_recovery_report(&["?? new.txt".to_string()]);

        assert!(report.contains("?? new.txt"));
        assert!(report.contains("git diff"));
        assert!(
            report.contains("不会自动删除或重置文件")
                || report.contains("does not automatically reset or delete files")
        );
    }
}
