use crate::config::Config;

use anyhow::Result;
use serde_json::Value;
use std::{env, fs, path::PathBuf};

struct Check {
    name: &'static str,
    status: &'static str,
    detail: String,
    suggestion: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: "ok",
            detail: detail.into(),
            suggestion: None,
        }
    }

    fn warning(
        name: &'static str,
        detail: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name,
            status: "warning",
            detail: detail.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    fn error(name: &'static str, detail: impl Into<String>, suggestion: impl Into<String>) -> Self {
        Self {
            name,
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

    let model_check = match serde_yaml::from_str::<serde_yaml::Value>(&content) {
        Ok(value) => value
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
            }),
        Err(err) => Check::warning(
            "Model",
            format!("not checked because config YAML is invalid: {err}"),
            "Fix ~/.aicmd/config.yaml, then run: aicmd doctor",
        ),
    };

    vec![Check::ok("Config", path.display().to_string()), model_check]
}

fn mcp_config_path() -> PathBuf {
    env::var("AICMD_MCP_CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Config::config_dir().join("mcp.json"))
}

fn check_mcp() -> Vec<Check> {
    let path = mcp_config_path();
    if !path.exists() {
        return vec![
            Check::warning(
                "MCP config",
                format!("not found at {}", path.display()),
                "Create ~/.aicmd/mcp.json or place mcp.json next to .env and run: aicmd init --from-env --force",
            ),
            Check::warning(
                "Search",
                "not checked because MCP config is missing",
                "Configure the search command in ~/.aicmd/mcp.json",
            ),
        ];
    }

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            return vec![
                Check::warning(
                    "MCP config",
                    format!("unable to read {}: {err}", path.display()),
                    "Check file permissions for ~/.aicmd/mcp.json",
                ),
                Check::warning(
                    "Search",
                    "not checked because MCP config cannot be read",
                    "Fix ~/.aicmd/mcp.json, then run: aicmd doctor",
                ),
            ];
        }
    };

    let parsed: Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(err) => {
            return vec![
                Check::warning(
                    "MCP config",
                    format!("invalid JSON at {}: {err}", path.display()),
                    "Fix ~/.aicmd/mcp.json JSON syntax",
                ),
                Check::warning(
                    "Search",
                    "not checked because MCP config JSON is invalid",
                    "Fix ~/.aicmd/mcp.json, then run: aicmd doctor",
                ),
            ];
        }
    };

    let root = parsed.get("mcp").unwrap_or(&parsed);
    let search = root
        .get("commands")
        .and_then(|commands| commands.get("search"))
        .is_some();
    let search_check = if search {
        Check::ok("Search", "configured")
    } else {
        Check::warning(
            "Search",
            "command not configured",
            "Add mcp.commands.search to ~/.aicmd/mcp.json",
        )
    };

    vec![
        Check::ok("MCP config", path.display().to_string()),
        search_check,
    ]
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
