# AICmd Execution Preflight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add local, read-only execution preflight checks that stop commands before confirmation when required dependencies, paths, environment variables, operating systems, or Git state do not satisfy the task.

**Architecture:** Extend both structured planner responses and explicit command-generation responses with the same strict `Vec<PreflightCheck>` contract. A focused `preflight_cmd` module validates and runs checks, while `main.rs` invokes it before the existing risk and confirmation flow. Successful command-cache records retain their checks so cached commands remain subject to preflight.

**Tech Stack:** Rust 2021, serde/serde_json, std::fs, std::env, std::process::Command, existing AICmd config/session/cache helpers.

## Global Constraints

- Support macOS and Linux; Windows users run AICmd inside WSL.
- Do not add dependencies.
- Keep one model request per task.
- Preflight checks are read-only and never install, repair, elevate privileges, or modify files.
- A failed or errored required check stops execution before confirmation.
- `--dry-run` and `--print` never run checks.
- Terminal labels obey `language: zh|en`, defaulting to Chinese.
- Unknown JSON fields, unknown check types, missing fields, and empty fields are rejected.
- Do not infer checks from generated command strings.

## File map

- Create `src/preflight_cmd.rs`: check schema, validation, local execution, result formatting.
- Modify `src/main.rs`: register the module, parse structured generated commands, run checks before confirmation.
- Modify `src/plan_cmd.rs`: add preflight checks to `ExecutionPlan` and add a strict generated-command parser.
- Modify `src/command_cache.rs`: persist checks with successful cached commands and ignore legacy entries without preflight metadata.
- Modify `src/result_cmd.rs`: build session notes for failed checks.
- Modify `assets/roles/%shell%.md`: require preflight in default planner JSON.
- Modify `assets/roles/%shell-command%.md`: return strict JSON containing `command` and `preflight`.
- Modify `README.md`, `README.en.md`, and `docs/aicmd-usage.md`: document behavior and supported checks.

---

### Task 1: Define and validate the preflight contract

**Files:**
- Create: `src/preflight_cmd.rs`
- Modify: `src/main.rs`
- Modify: `src/plan_cmd.rs`

**Interfaces:**
- Produces: `PreflightCheck`, `PreflightType`, `validate_checks(&[PreflightCheck]) -> Result<()>`.
- Consumed later by: plan parsing, command generation, cache records, and the runner.

- [ ] **Step 1: Register the new module**

Add beside the existing command modules in `src/main.rs`:

```rust
mod preflight_cmd;
```

- [ ] **Step 2: Write failing schema-validation tests**

Create `src/preflight_cmd.rs` with the public types and tests first:

```rust
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
        if check.value.contains("$(")
            || check.value.contains('`')
            || check.value.contains("${")
        {
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
    }

    #[test]
    fn serde_rejects_unknown_fields_and_check_types() {
        let unknown_type = r#"{"type":"network","value":"x","failure_message":"f","suggestion":"s"}"#;
        let unknown_field = r#"{"type":"path_exists","value":"x","failure_message":"f","suggestion":"s","extra":true}"#;
        assert!(serde_json::from_str::<PreflightCheck>(unknown_type).is_err());
        assert!(serde_json::from_str::<PreflightCheck>(unknown_field).is_err());
    }
}
```

- [ ] **Step 3: Run the focused tests and verify they pass**

Run:

```bash
cargo test preflight_cmd::tests -- --nocapture
```

Expected: both new tests pass.

- [ ] **Step 4: Add preflight to the strict plan parser**

In `src/plan_cmd.rs`, import and add the field:

```rust
use crate::preflight_cmd::{validate_checks, PreflightCheck};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub mode: PlanMode,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub problem: String,
    pub preflight: Vec<PreflightCheck>,
}
```

Call `validate_checks(&plan.preflight)?` in `parse_execution_plan`. Reject
non-empty preflight arrays for `search` and `diagnose`; their later executable
command is generated through `GeneratedCommand` and carries its own checks.
Allow preflight arrays for `direct` and `script`.

Update parser fixtures to include `"preflight":[]`:

```rust
let raw = r#"{"mode":"direct","command":"pwd","query":"","problem":"","preflight":[]}"#;
assert_eq!(parse_execution_plan(raw)?.mode, PlanMode::Direct);
assert!(parse_execution_plan(
    r#"{"mode":"search","command":"","query":"rust","problem":"","preflight":[{"type":"command_exists","value":"git","failure_message":"f","suggestion":"s"}]}"#
).is_err());
```

- [ ] **Step 5: Run plan and preflight tests**

Run:

```bash
cargo test plan_cmd::tests preflight_cmd::tests
```

Expected: all selected tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/plan_cmd.rs src/preflight_cmd.rs
git commit -m "feat: define execution preflight contract"
```

---

### Task 2: Implement the local read-only check runner

**Files:**
- Modify: `src/preflight_cmd.rs`

**Interfaces:**
- Consumes: `PreflightCheck`, `PreflightType`.
- Produces: `PreflightFailure`, `PreflightReport`, `run_checks(&[PreflightCheck], &Path) -> PreflightReport`, `format_report(&PreflightReport) -> String`.

- [ ] **Step 1: Write failing runner tests**

Add temporary-directory tests using only `std`:

```rust
#[test]
fn reports_all_failures_in_input_order() {
    let root = std::env::temp_dir().join(format!("aicmd-preflight-{}", uuid::Uuid::new_v4()));
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
fn env_check_never_exposes_the_value() {
    std::env::set_var("AICMD_PREFLIGHT_SECRET", "do-not-print");
    let report = run_checks(
        &[check(PreflightType::EnvExists, "AICMD_PREFLIGHT_SECRET")],
        std::path::Path::new("."),
    );
    assert!(report.failures.is_empty());
    assert!(!format_report(&report).contains("do-not-print"));
    std::env::remove_var("AICMD_PREFLIGHT_SECRET");
}
```

Add these deterministic cases:

```rust
#[test]
fn checks_command_path_writable_env_and_os() {
    let root = std::env::temp_dir().join(format!("aicmd-preflight-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("present")).unwrap();
    let checks = vec![
        check(PreflightType::CommandExists, "sh"),
        check(PreflightType::PathExists, "present"),
        check(PreflightType::PathWritable, "new/output.txt"),
        check(PreflightType::Os, std::env::consts::OS),
    ];
    assert!(run_checks(&checks, &root).passed());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn detects_clean_and_dirty_git_repositories() {
    let root = std::env::temp_dir().join(format!("aicmd-preflight-git-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    assert!(Command::new("git").arg("-C").arg(&root).arg("init").status().unwrap().success());
    assert!(run_checks(&[check(PreflightType::GitClean, ".")], &root).passed());
    std::fs::write(root.join("dirty.txt"), "dirty").unwrap();
    assert!(!run_checks(&[check(PreflightType::GitClean, ".")], &root).passed());
    std::fs::remove_dir_all(root).unwrap();
}
```

- [ ] **Step 2: Run the tests and verify failure**

Run:

```bash
cargo test preflight_cmd::tests -- --nocapture
```

Expected: compilation fails because `run_checks`, `PreflightReport`, and
`format_report` do not exist.

- [ ] **Step 3: Implement result types and path resolution**

Add:

```rust
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

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
```

- [ ] **Step 4: Implement each check without modifying state**

Use these helpers:

```rust
fn command_exists(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| {
            let path = dir.join(name);
            path.metadata()
                .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
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
```

Implement `run_one(check, cwd) -> Result<(), String>`:

- `CommandExists`: scan `PATH`.
- `PathExists`: call `resolve_path(...).exists()`.
- `PathWritable`: call `path_writable`.
- `EnvExists`: call `env::var_os(&check.value).is_some()`.
- `Os`: compare `env::consts::OS`.
- `GitClean`: run `git -C <path> rev-parse --is-inside-work-tree`, then
  `git -C <path> status --porcelain`; fail when either command errors or status
  output is non-empty.

`run_checks` must execute every item and collect all failures:

```rust
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
```

- [ ] **Step 5: Implement language-aware formatting**

Use the existing `localized` helper:

```rust
pub fn format_report(report: &PreflightReport) -> String {
    if report.passed() {
        return if crate::utils::is_chinese() {
            format!("执行前检查：通过（{} 项）", report.total)
        } else {
            format!("Preflight: passed ({} checks)", report.total)
        };
    }
    let mut lines = vec![crate::utils::localized(
        "执行前检查失败",
        "Preflight failed",
    )
    .to_string()];
    for failure in &report.failures {
        lines.push(format!("\n✗ {}", failure.check.failure_message));
        lines.push(format!(
            "  {}：{} = {}",
            crate::utils::localized("检查", "Check"),
            serde_json::to_value(&failure.check.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_default(),
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
```

- [ ] **Step 6: Run focused and full tests**

Run:

```bash
cargo test preflight_cmd::tests -- --nocapture
cargo test
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/preflight_cmd.rs
git commit -m "feat: run local execution preflight checks"
```

---

### Task 3: Use structured preflight for every model-generated command

**Files:**
- Modify: `src/plan_cmd.rs`
- Modify: `src/main.rs`
- Modify: `assets/roles/%shell-command%.md`

**Interfaces:**
- Produces: `GeneratedCommand { command: String, preflight: Vec<PreflightCheck> }`, `parse_generated_command(&str) -> Result<GeneratedCommand>`.
- Consumed by: `shell_execute` for explicit `do`, `err`, diagnosis, revision, and repair flows.

- [ ] **Step 1: Write failing generated-command parser tests**

In `src/plan_cmd.rs` add tests:

```rust
#[test]
fn parses_strict_generated_command_with_preflight() -> anyhow::Result<()> {
    let generated = parse_generated_command(
        r#"{"command":"python3 task.py","preflight":[{"type":"command_exists","value":"python3","failure_message":"未找到 Python 3","suggestion":"请先安装 Python 3"}]}"#,
    )?;
    assert_eq!(generated.command, "python3 task.py");
    assert_eq!(generated.preflight.len(), 1);
    assert!(parse_generated_command(r#"{"command":"pwd","preflight":[],"extra":1}"#).is_err());
    assert!(parse_generated_command(r#"{"command":"","preflight":[]}"#).is_err());
    Ok(())
}
```

- [ ] **Step 2: Implement the strict generated-command type**

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCommand {
    pub command: String,
    pub preflight: Vec<PreflightCheck>,
}

pub fn parse_generated_command(raw: &str) -> Result<GeneratedCommand> {
    let generated: GeneratedCommand = serde_json::from_str(raw)?;
    if generated.command.trim().is_empty() {
        bail!("generated command must not be empty");
    }
    validate_checks(&generated.preflight)?;
    Ok(generated)
}
```

- [ ] **Step 3: Change `shell_execute` to request raw JSON**

In `src/main.rs`, import:

```rust
use crate::plan_cmd::{
    parse_generated_command, request_execution_plan, route_kind, ExecutionPlan, RouteKind,
};
```

Replace the plain command completion in `shell_execute`:

```rust
let (raw, _) =
    call_chat_completions_raw(&input, client.as_ref(), abort_signal.clone()).await?;
let generated = parse_generated_command(&raw)
    .context(localized("无效命令计划", "Invalid command plan"))?;
```

Pass both fields to `ShellExecutionOptions`:

```rust
ShellExecutionOptions {
    eval_str: generated.command,
    preflight: generated.preflight,
    cache_task,
    record_assistant_message: true,
    repair_attempts,
    from_cache: false,
    ask_summary,
}
```

Add `preflight: Vec<PreflightCheck>` to `ShellExecutionOptions`.

- [ ] **Step 4: Update the command role contract**

Rewrite the output contract at the top of `assets/roles/%shell-command%.md`:

```text
Output exactly one JSON object and nothing else:
{"command":"<valid shell command>","preflight":[]}

只输出一个 JSON 对象，不要输出其他内容：
{"command":"<有效 shell 命令>","preflight":[]}

`preflight` contains only required read-only checks. Use an empty array for a
simple dependency-free read-only command. Each item must contain exactly
`type`, `value`, `failure_message`, and `suggestion`.
```

Include one valid `command_exists` example and one `path_exists` example.
State that install tasks check the package manager, not the package being
installed.

- [ ] **Step 5: Update command-role tests**

In `src/config/role.rs`, extend
`builtin_shell_roles_separate_planning_from_command_generation` to assert:

```rust
assert!(command.prompt.contains("\"preflight\""));
assert!(command.prompt.contains("\"command\""));
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test plan_cmd::tests config::role::tests
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/plan_cmd.rs src/main.rs assets/roles/%shell-command%.md src/config/role.rs
git commit -m "feat: structure generated commands with preflight"
```

---

### Task 4: Run preflight before confirmation and preserve it in cache

**Files:**
- Modify: `src/main.rs`
- Modify: `src/command_cache.rs`
- Modify: `src/result_cmd.rs`

**Interfaces:**
- Consumes: `run_checks`, `format_report`, `Vec<PreflightCheck>`.
- Produces: cache records with preflight metadata and session notes for failed checks.

- [ ] **Step 1: Write cache compatibility tests**

In `src/command_cache.rs`, import `PreflightCheck` and add:

```rust
#[test]
fn legacy_cache_record_without_preflight_is_not_reusable() {
    let yaml = r#"
- key: old
  task: pwd
  shell: zsh
  os: macos
  command: pwd
  success_count: 1
  last_used_at: now
"#;
    let records: Vec<CommandCacheRecord> = serde_yaml::from_str(yaml).unwrap();
    assert!(!records[0].has_preflight);
}
```

- [ ] **Step 2: Extend the cache record**

Add:

```rust
pub struct CommandCacheRecord {
    pub key: String,
    pub task: String,
    pub shell: String,
    pub os: String,
    pub command: String,
    pub success_count: u32,
    pub last_used_at: String,
    #[serde(default)]
    pub preflight: Vec<PreflightCheck>,
    #[serde(default)]
    pub has_preflight: bool,
}
```

Change:

```rust
pub fn record_success(
    task: &str,
    shell: &str,
    os: &str,
    command: &str,
    preflight: &[PreflightCheck],
) -> Result<()>
```

Store `preflight.to_vec()` and `has_preflight: true`. Change `lookup` to ignore
records where `has_preflight` is false, forcing old cache entries through the
planner once before they can be reused.

- [ ] **Step 3: Write the failed-preflight session-note test**

In `src/result_cmd.rs`:

```rust
#[test]
fn builds_preflight_failure_note_without_environment_values() {
    let report = PreflightReport {
        total: 1,
        failures: vec![PreflightFailure {
            check: PreflightCheck {
                kind: PreflightType::EnvExists,
                value: "API_TOKEN".to_string(),
                failure_message: "缺少环境变量".to_string(),
                suggestion: "请配置 API_TOKEN".to_string(),
            },
            detail: "not set".to_string(),
        }],
    };
    let note = build_preflight_session_note("deploy", &report);
    assert!(note.contains("API_TOKEN"));
    assert!(!note.contains("secret-value"));
}
```

- [ ] **Step 4: Implement the session note**

Add:

```rust
pub fn build_preflight_session_note(task: &str, report: &PreflightReport) -> String {
    let failures = report
        .failures
        .iter()
        .map(|failure| {
            format!(
                "- {:?}: {} | {}",
                failure.check.kind,
                failure.check.value,
                failure.check.failure_message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Execution preflight failed:\nTask:\n{task}\n\nFailures:\n{failures}"
    )
}
```

- [ ] **Step 5: Run checks once before the interactive confirmation loop**

In `handle_generated_command`, after empty-command and `--print` handling but
before creating the client or displaying risk:

```rust
let cwd = env::current_dir().context("Unable to read current directory")?;
let report = preflight_cmd::run_checks(&preflight, &cwd);
println!("{}", preflight_cmd::format_report(&report));
if !report.passed() {
    let task = cache_task.as_deref().unwrap_or_else(|| input.text().as_str());
    let note = result_cmd::build_preflight_session_note(task, &report);
    config.write().append_session_note(note)?;
    return Ok(());
}
```

Store `input.text()` in a local `String` before selecting `task` so no borrowed
temporary crosses the session write.

Pass the same `preflight` to `command_cache::record_success` after successful
execution.

- [ ] **Step 6: Pass plan and cached checks into execution options**

For default plan routing:

```rust
preflight: plan.preflight,
```

For cache reuse:

```rust
let cached = cached_record.expect("cache hit has a record");
eval_str: cached.command,
preflight: cached.preflight,
```

Update the cache lookup variable from `Option<String>` to
`Option<CommandCacheRecord>`.

- [ ] **Step 7: Run full tests**

Run:

```bash
cargo test
```

Expected: all tests pass, including cache migration and preflight report tests.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/command_cache.rs src/result_cmd.rs
git commit -m "feat: stop command execution on failed preflight"
```

---

### Task 5: Update the default planner prompt and terminal-language behavior

**Files:**
- Modify: `assets/roles/%shell%.md`
- Modify: `src/config/role.rs`
- Modify: `src/preflight_cmd.rs`

**Interfaces:**
- Consumes: existing role template variables and `language: zh|en`.
- Produces: valid planner JSON for all modes and one-language terminal output.

- [ ] **Step 1: Update the planner JSON contract**

Change the exact-field instruction in `assets/roles/%shell%.md` to:

```text
The object has exactly these fields: `mode`, `command`, `query`, `problem`,
and `preflight`. `preflight` is an array. For `direct`, `script`, and
`diagnose`, include every required read-only check. For `search`, use an empty
array. Do not add fields.
```

Update all four examples. Direct example:

```json
{"mode":"direct","command":"docker ps","query":"","problem":"","preflight":[{"type":"command_exists","value":"docker","failure_message":"未找到 Docker","suggestion":"请先安装并启动 Docker"}]}
```

Add these planner rules:

- use empty preflight for dependency-free read-only commands such as `pwd`;
- do not check for the target package during an install task;
- check the required package manager or input file instead;
- never put commands, shell expansion, or secrets in check values;
- generate `failure_message` and `suggestion` in the configured language.

- [ ] **Step 2: Add role-contract assertions**

In `src/config/role.rs`:

```rust
assert!(planner.prompt.contains("\"preflight\""));
assert!(planner.prompt.contains("command_exists"));
assert!(planner.prompt.contains("path_exists"));
```

- [ ] **Step 3: Add formatting assertions**

In `src/preflight_cmd.rs`, test only one language appears by setting a temporary
config through `AICMD_CONFIG_FILE`, formatting a failed report, and asserting:

```rust
assert!(zh.contains("执行前检查失败"));
assert!(!zh.contains("Preflight failed"));
assert!(en.contains("Preflight failed"));
assert!(!en.contains("执行前检查失败"));
```

Serialize environment-variable mutation with a static `Mutex<()>`, matching
the existing project pattern if one exists; otherwise keep the test scoped to
one function and restore the previous environment variable afterward.

- [ ] **Step 4: Run role and formatting tests**

Run:

```bash
cargo test config::role::tests preflight_cmd::tests
```

Expected: all selected tests pass.

- [ ] **Step 5: Commit**

```bash
git add assets/roles/%shell%.md src/config/role.rs src/preflight_cmd.rs
git commit -m "feat: teach planner to declare preflight checks"
```

---

### Task 6: Documentation and end-to-end verification

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/aicmd-usage.md`

**Interfaces:**
- Documents the completed behavior; no new runtime interface.

- [ ] **Step 1: Update Chinese README**

Add a concise “执行前检查” section after the ordinary command workflow:

```text
AICmd 会在执行确认前检查任务声明的必要条件。支持检查命令、路径、写入权限、
环境变量是否存在、操作系统和 Git 工作区状态。检查只读执行；任一检查失败时，
AICmd 会显示全部原因和建议，并且不会执行命令。
```

Document:

- `--dry-run` shows the plan and checks without running them;
- `--print` prints only the command;
- checks do not automatically install or repair dependencies.

- [ ] **Step 2: Update English README**

Add the equivalent concise English section:

```text
AICmd checks plan-declared requirements before execution confirmation. Checks
are read-only. If any required check fails, AICmd shows all reasons and
suggestions and does not execute the command.
```

- [ ] **Step 3: Update detailed usage documentation**

Add the six supported check types and one valid JSON example to
`docs/aicmd-usage.md`. Explicitly state that environment-variable values are
never displayed or saved.

- [ ] **Step 4: Review documentation with `karpathy-guidelines`**

Verify:

- no repeated feature tour;
- no claim of automatic repair;
- no native Windows support claim;
- examples match the implemented JSON fields;
- Chinese and English README files remain separate.

- [ ] **Step 5: Run project verification**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy --all --all-targets -- -D warnings
cargo build --release
git diff --check
```

Expected: every command exits with code 0.

- [ ] **Step 6: Run manual smoke checks**

Run:

```bash
./target/release/aicmd --dry-run "读取 missing.csv 并生成 output.csv"
./target/release/aicmd --print "当前目录有多少文件"
./target/release/aicmd "读取 missing.csv 并生成 output.csv"
```

Verify:

- dry-run JSON contains `preflight`;
- print output contains only the command;
- missing path reports a failure and never displays `执行？` or `Run?`.

- [ ] **Step 7: Commit documentation**

```bash
git add README.md README.en.md docs/aicmd-usage.md
git commit -m "docs: explain execution preflight checks"
```

- [ ] **Step 8: Install the verified local release**

Run:

```bash
install -m 0755 target/release/aicmd "$HOME/.local/bin/aicmd"
"$HOME/.local/bin/aicmd" --version
```

Expected: the installed binary reports the current package version.
