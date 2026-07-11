use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

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
}
