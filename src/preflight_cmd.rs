use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightType {
    CommandExists,
    PathExists,
    PathWritable,
    EnvExists,
    Os,
    GitClean,
}

impl PreflightType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::CommandExists => "command_exists",
            Self::PathExists => "path_exists",
            Self::PathWritable => "path_writable",
            Self::EnvExists => "env_exists",
            Self::Os => "os",
            Self::GitClean => "git_clean",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightCheck {
    #[serde(rename = "type")]
    pub kind: PreflightType,
    pub value: String,
    pub failure_message: String,
    pub suggestion: String,
}

pub fn validate_checks(checks: &[PreflightCheck]) -> Result<()> {
    for check in checks {
        if check.value.trim().is_empty()
            || check.failure_message.trim().is_empty()
            || check.suggestion.trim().is_empty()
        {
            bail!("preflight fields must not be empty");
        }
        if matches!(check.kind, PreflightType::Os)
            && !matches!(check.value.as_str(), "macos" | "linux")
        {
            bail!("preflight os must be macos or linux");
        }
        if check.value.contains('$') || check.value.contains('`') {
            bail!("preflight value cannot contain shell expansion");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightFailure {
    pub check: PreflightCheck,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub total: usize,
    pub failures: Vec<PreflightFailure>,
}

impl PreflightReport {
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

fn resolve_path(value: &str, cwd: &Path) -> PathBuf {
    let expanded = crate::utils::resolve_home_dir(value);
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn command_exists(name: &str) -> bool {
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| {
            dir.join(name).metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    })
}

fn nearest_existing_parent(mut path: PathBuf) -> Option<PathBuf> {
    while !path.exists() {
        path = path.parent()?.to_path_buf();
    }
    Some(path)
}

fn path_writable(path: &Path) -> bool {
    nearest_existing_parent(path.to_path_buf()).is_some_and(|existing| {
        Command::new("test")
            .arg("-w")
            .arg(existing)
            .status()
            .is_ok_and(|status| status.success())
    })
}

fn git_clean(path: &Path) -> Result<(), String> {
    let inside = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|err| err.to_string())?;
    if !inside.status.success() {
        return Err("not a git repository".to_string());
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|err| err.to_string())?;
    if !status.status.success() {
        return Err("unable to inspect git status".to_string());
    }
    if !status.stdout.is_empty() {
        return Err("git working tree is not clean".to_string());
    }
    Ok(())
}

fn run_one(check: &PreflightCheck, cwd: &Path) -> Result<(), String> {
    let fail = |detail: &str| Err(detail.to_string());
    match check.kind {
        PreflightType::CommandExists => {
            if command_exists(&check.value) {
                Ok(())
            } else {
                fail("command not found")
            }
        }
        PreflightType::PathExists => {
            if resolve_path(&check.value, cwd).exists() {
                Ok(())
            } else {
                fail("path does not exist")
            }
        }
        PreflightType::PathWritable => {
            if path_writable(&resolve_path(&check.value, cwd)) {
                Ok(())
            } else {
                fail("path is not writable")
            }
        }
        PreflightType::EnvExists => {
            if env::var_os(&check.value).is_some() {
                Ok(())
            } else {
                fail("environment variable is not set")
            }
        }
        PreflightType::Os => {
            if env::consts::OS == check.value {
                Ok(())
            } else {
                fail("operating system does not match")
            }
        }
        PreflightType::GitClean => git_clean(&resolve_path(&check.value, cwd)),
    }
}

pub fn run_checks(checks: &[PreflightCheck], cwd: &Path) -> PreflightReport {
    let failures = checks
        .iter()
        .filter_map(|check| {
            run_one(check, cwd).err().map(|detail| PreflightFailure {
                check: check.clone(),
                detail,
            })
        })
        .collect();
    PreflightReport {
        total: checks.len(),
        failures,
    }
}

pub fn format_report(report: &PreflightReport) -> String {
    if report.passed() {
        return if crate::utils::is_chinese() {
            format!("执行前检查：通过（{} 项）", report.total)
        } else {
            format!("Preflight: passed ({} checks)", report.total)
        };
    }
    let mut lines = vec![crate::utils::localized("执行前检查失败", "Preflight failed").to_string()];
    for failure in &report.failures {
        lines.push(format!("\n✗ {}", failure.check.failure_message));
        lines.push(format!(
            "  {}：{} = {}",
            crate::utils::localized("检查", "Check"),
            failure.check.kind.as_str(),
            failure.check.value
        ));
        lines.push(format!(
            "  {}：{}",
            crate::utils::localized("建议", "Suggestion"),
            failure.check.suggestion
        ));
    }
    lines.push(format!(
        "\n{}",
        crate::utils::localized("命令未执行。", "Command was not executed.")
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(kind: PreflightType, value: &str) -> PreflightCheck {
        PreflightCheck {
            kind,
            value: value.to_string(),
            failure_message: "failed".to_string(),
            suggestion: "fix it".to_string(),
        }
    }

    #[test]
    fn validates_supported_checks_and_rejects_unsafe_values() {
        assert!(validate_checks(&[check(PreflightType::CommandExists, "git")]).is_ok());
        assert!(validate_checks(&[check(PreflightType::Os, "windows")]).is_err());
        assert!(validate_checks(&[check(PreflightType::PathExists, "$(pwd)")]).is_err());
        assert!(validate_checks(&[check(PreflightType::EnvExists, "$TOKEN")]).is_err());
    }

    #[test]
    fn serde_rejects_unknown_fields_and_check_types() {
        let unknown_type =
            r#"{"type":"network","value":"x","failure_message":"f","suggestion":"s"}"#;
        let unknown_field = r#"{"type":"path_exists","value":"x","failure_message":"f","suggestion":"s","extra":true}"#;
        assert!(serde_json::from_str::<PreflightCheck>(unknown_type).is_err());
        assert!(serde_json::from_str::<PreflightCheck>(unknown_field).is_err());
    }

    #[test]
    fn reports_all_failures_in_input_order() {
        let root = env::temp_dir().join(format!("aicmd-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let checks = vec![
            check(PreflightType::PathExists, "missing-a"),
            check(PreflightType::PathExists, "missing-b"),
        ];
        let report = run_checks(&checks, &root);
        assert_eq!(report.total, 2);
        assert_eq!(report.failures.len(), 2);
        assert_eq!(report.failures[0].check.value, "missing-a");
        assert_eq!(report.failures[1].check.value, "missing-b");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checks_command_path_writable_env_and_os() {
        let root = env::temp_dir().join(format!("aicmd-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("present")).unwrap();
        let variable = format!("AICMD_PREFLIGHT_{}", uuid::Uuid::new_v4().simple());
        env::set_var(&variable, "do-not-print");
        let checks = vec![
            check(PreflightType::CommandExists, "sh"),
            check(PreflightType::PathExists, "present"),
            check(PreflightType::PathWritable, "new/output.txt"),
            check(PreflightType::EnvExists, &variable),
            check(PreflightType::Os, env::consts::OS),
        ];
        let report = run_checks(&checks, &root);
        assert!(report.passed(), "{:?}", report.failures);
        assert!(!format_report(&report).contains("do-not-print"));
        env::remove_var(variable);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_clean_and_dirty_git_repositories() {
        let root = env::temp_dir().join(format!("aicmd-preflight-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("init")
            .status()
            .unwrap()
            .success());
        assert!(run_checks(&[check(PreflightType::GitClean, ".")], &root).passed());
        std::fs::write(root.join("dirty.txt"), "dirty").unwrap();
        assert!(!run_checks(&[check(PreflightType::GitClean, ".")], &root).passed());
        std::fs::remove_dir_all(root).unwrap();
    }
}
