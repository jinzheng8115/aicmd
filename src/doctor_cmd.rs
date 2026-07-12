use crate::{config::Config, mcp_cmd};

use anyhow::Result;
use std::{env, fs, path::PathBuf};

struct Check {
    name: String,
    status: &'static str,
    detail: String,
    suggestion: Option<String>,
}

impl Check {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "ok",
            detail: detail.into(),
            suggestion: None,
        }
    }

    fn warning(
        name: impl Into<String>,
        detail: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: "warning",
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    fn error(
        name: impl Into<String>,
        detail: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: "error",
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

pub fn run_doctor_command() -> Result<i32> {
    let mut checks = vec![];

    checks.push(check_binary());
    checks.push(Check::ok("Version", env!("CARGO_PKG_VERSION")));
    checks.extend(check_config());
    checks.extend(check_mcp());
    checks.push(check_command_cache());
    checks.push(check_searches_dir());
    checks.push(check_path());
    checks.push(check_shell_integration());

    println!("AICmd doctor\n");
    for check in &checks {
        println!("{}: {} {}", check.name, check.status, check.detail);
    }

    let suggestions: Vec<_> = checks
        .iter()
        .filter_map(|check| check.suggestion.as_deref())
        .collect();
    if !suggestions.is_empty() {
        println!("\nSuggestions:");
        for suggestion in suggestions {
            println!("- {suggestion}");
        }
    }

    Ok(0)
}

fn check_binary() -> Check {
    match env::current_exe() {
        Ok(path) => Check::ok("Binary", path.display().to_string()),
        Err(err) => Check::warning(
            "Binary",
            format!("unable to resolve current executable: {err}"),
            "Run: which aicmd",
        ),
    }
}

fn check_config() -> Vec<Check> {
    let path = Config::config_file();
    if !path.exists() {
        return vec![
            Check::error(
                "Config",
                format!("not found at {}", path.display()),
                "Prepare .env, then run: aicmd init --from-env",
            ),
            Check::warning(
                "Model",
                "not checked because config is missing",
                "Create config first: aicmd init --from-env",
            ),
        ];
    }

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            return vec![
                Check::warning(
                    "Config",
                    format!("unable to read {}: {err}", path.display()),
                    "Check file permissions for ~/.aicmd/config.yaml",
                ),
                Check::warning(
                    "Model",
                    "not checked because config cannot be read",
                    "Fix config permissions, then run: aicmd doctor",
                ),
            ];
        }
    };

    match serde_yaml::from_str::<serde_yaml::Value>(&content) {
        Ok(value) => {
            let model_check = value
                .get("model")
                .and_then(|model| model.as_str())
                .filter(|model| !model.trim().is_empty())
                .map(|model| Check::ok("Model", model.to_string()))
                .unwrap_or_else(|| {
                    Check::warning(
                        "Model",
                        "missing model field",
                        "Set model in ~/.aicmd/config.yaml",
                    )
                });
            vec![
                Check::ok("Config", path.display().to_string()),
                model_check,
                check_temperature(&value),
                check_ai_summary(&value),
            ]
        }
        Err(err) => vec![
            Check::ok("Config", path.display().to_string()),
            Check::warning(
                "Model",
                format!("not checked because config YAML is invalid: {err}"),
                "Fix ~/.aicmd/config.yaml, then run: aicmd doctor",
            ),
            Check::warning(
                "Temperature",
                "not checked because config YAML is invalid",
                "Fix ~/.aicmd/config.yaml, then run: aicmd doctor",
            ),
            Check::warning(
                "AI summary",
                "not checked because config YAML is invalid",
                "Fix ~/.aicmd/config.yaml, then run: aicmd doctor",
            ),
        ],
    }
}

fn check_temperature(value: &serde_yaml::Value) -> Check {
    match value.get("temperature") {
        Some(v) if v.is_null() => Check::warning(
            "Temperature",
            "provider default",
            "For more deterministic commands, set temperature: 0 or regenerate config: aicmd init --from-env --force",
        ),
        Some(v) if v.as_f64() == Some(0.0) => Check::ok("Temperature", "0"),
        Some(v) => Check::warning(
            "Temperature",
            yaml_scalar_display(v),
            "For more deterministic commands, set temperature: 0",
        ),
        None => Check::warning(
            "Temperature",
            "missing",
            "For more deterministic commands, add temperature: 0 to ~/.aicmd/config.yaml",
        ),
    }
}

fn check_ai_summary(value: &serde_yaml::Value) -> Check {
    match value.get("ai_summary").and_then(|v| v.as_bool()) {
        Some(true) => Check::ok("AI summary", "on"),
        Some(false) => Check::ok("AI summary", "off"),
        None => Check::warning(
            "AI summary",
            "missing, defaults to off; ask after execution",
            "Run: aicmd config summary on   # to summarize automatically",
        ),
    }
}

fn yaml_scalar_display(value: &serde_yaml::Value) -> String {
    if value.is_null() {
        "null".to_string()
    } else if let Some(text) = value.as_str() {
        text.to_string()
    } else if let Some(number) = value.as_f64() {
        number.to_string()
    } else if let Some(enabled) = value.as_bool() {
        enabled.to_string()
    } else {
        "non-scalar value".to_string()
    }
}

fn check_mcp() -> Vec<Check> {
    mcp_checks(mcp_cmd::diagnose_config())
}

fn mcp_checks(diagnostics: Vec<mcp_cmd::McpDiagnostic>) -> Vec<Check> {
    diagnostics
        .into_iter()
        .map(|diagnostic| Check {
            name: diagnostic.name,
            status: diagnostic.status,
            detail: diagnostic.detail,
            suggestion: diagnostic.suggestion,
        })
        .collect()
}

fn check_command_cache() -> Check {
    let path = Config::local_path("command-cache.yaml");
    if !path.exists() {
        return Check::ok(
            "Command cache",
            format!(
                "missing, will be created after first successful command ({})",
                path.display()
            ),
        );
    }
    match fs::read_to_string(&path) {
        Ok(content) if content.trim().is_empty() => Check::warning(
            "Command cache",
            format!("empty at {}", path.display()),
            "This is usually harmless; successful commands will repopulate it",
        ),
        Ok(content) => match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(_) => Check::ok("Command cache", path.display().to_string()),
            Err(err) => Check::warning(
                "Command cache",
                format!("invalid YAML at {}: {err}", path.display()),
                "Remove ~/.aicmd/command-cache.yaml to let AICmd recreate it",
            ),
        },
        Err(err) => Check::warning(
            "Command cache",
            format!("unable to read {}: {err}", path.display()),
            "Check file permissions for ~/.aicmd/command-cache.yaml",
        ),
    }
}

fn check_searches_dir() -> Check {
    let path = Config::local_path("searches");
    if path.is_dir() {
        Check::ok("Searches dir", path.display().to_string())
    } else if path.exists() {
        Check::warning(
            "Searches dir",
            format!("{} exists but is not a directory", path.display()),
            "Move or remove it, then run aicmd search again",
        )
    } else {
        Check::ok(
            "Searches dir",
            format!(
                "missing, will be created after first saved search ({})",
                path.display()
            ),
        )
    }
}

fn check_path() -> Check {
    let Ok(exe) = env::current_exe() else {
        return Check::warning(
            "PATH",
            "unable to inspect executable path",
            "Run: which aicmd",
        );
    };
    let Some(bin_dir) = exe.parent() else {
        return Check::warning(
            "PATH",
            "unable to inspect executable directory",
            "Run: which aicmd",
        );
    };
    let path_value = env::var_os("PATH").unwrap_or_default();
    if env::split_paths(&path_value).any(|path| path == bin_dir) {
        Check::ok("PATH", bin_dir.display().to_string())
    } else {
        Check::warning(
            "PATH",
            format!("{} not found in PATH", bin_dir.display()),
            format!("Add {} to PATH", bin_dir.display()),
        )
    }
}

fn check_shell_integration() -> Check {
    if env::var("AICMD_SHELL_INTEGRATION").as_deref() == Ok("1") {
        return Check::ok("Shell integration", "active in current shell");
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let candidates = [
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".profile"),
    ];
    for path in candidates {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        if content.contains("aicmd shell-init") {
            return Check::ok("Shell integration", path.display().to_string());
        }
    }
    Check::warning(
        "Shell integration",
        "not detected",
        r#"Run: eval "$(aicmd shell-init)""#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_cmd::McpDiagnostic;

    #[test]
    fn mcp_diagnostics_become_individual_doctor_checks() {
        let checks = mcp_checks(vec![
            McpDiagnostic {
                name: "MCP server web".into(),
                status: "error",
                detail: "executable not found".into(),
                suggestion: Some("Install it".into()),
            },
            McpDiagnostic {
                name: "MCP command search".into(),
                status: "ok",
                detail: "server web".into(),
                suggestion: None,
            },
        ]);

        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].name, "MCP server web");
        assert_eq!(checks[0].status, "error");
        assert_eq!(checks[0].suggestion.as_deref(), Some("Install it"));
        assert_eq!(checks[1].name, "MCP command search");
        assert_eq!(checks[1].status, "ok");
    }

    #[test]
    fn search_doctor_check_keeps_legacy_states() {
        for (status, detail) in [
            ("ok", "configured"),
            ("warning", "command not configured"),
            ("warning", "not checked because MCP config is missing"),
        ] {
            let check = mcp_checks(vec![McpDiagnostic {
                name: "Search".into(),
                status,
                detail: detail.into(),
                suggestion: None,
            }])
            .pop()
            .unwrap();

            assert_eq!(check.name, "Search");
            assert_eq!(check.status, status);
            assert_eq!(check.detail, detail);
        }
    }
}
