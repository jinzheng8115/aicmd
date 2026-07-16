# AICmd Session Natural-Language Intents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users inspect, clear, and use named AICmd sessions through a small deterministic set of Chinese and English natural-language phrases while preserving the daily session as the default.

**Architecture:** Extend the existing pure parser in `src/intent_cmd.rs`. Read-only intents reuse `session_cmd::run_session_command` before model initialization; clear and named-session task intents translate into the existing `Cli.session`, `Cli.empty_session`, and `run` flow so confirmation, context, execution, and persistence remain shared.

**Tech Stack:** Rust 2021, existing Clap CLI, existing session/search intent helpers, serde YAML session storage, no new dependencies.

## Global Constraints

- Do not persist a new default session.
- A later plain `aicmd <task>` must still use the Beijing-date `cmd-YYYYMMDD` session.
- Do not add a CLI command, configuration field, state file, dependency, TUI, or model call.
- Recognized session intents must not pass through the model planner.
- Unmatched natural language must continue through the existing structured planner.
- Clearing any session must reuse the existing high-risk confirmation and display the resolved session name.
- Keep existing explicit `aicmd session ...`, `aicmd -s ...`, and `--empty-session` behavior compatible.
- Terminal language follows the existing `language: zh|en` behavior where the reused path already localizes output.
- Documentation remains Chinese and English and receives a `karpathy-guidelines` review.
- Do not stage local files such as `mcp.json`, `.env`, `.aicmd/`, `.DS_Store`, `.codebase-memory/`, or `tavily_key.txt`.

## File Structure

- Modify `src/intent_cmd.rs`: add typed session intent variants, exact parser helpers, and parser tests.
- Modify `src/main.rs`: route read-only intents and translate clear/named-session task intents into the existing CLI flow.
- Modify `README.md`: document Chinese natural-language session examples.
- Modify `README.en.md`: document matching English examples.
- Modify `docs/aicmd-usage.md`: add the detailed bilingual reference and behavior boundaries.
- Modify `docs/superpowers/plans/2026-06-19-command-simplification-plan.md`: mark Phase 4 complete and record the non-persistent session semantics.

---

### Task 1: Extend the deterministic session intent parser

**Files:**
- Modify: `src/intent_cmd.rs`

**Interfaces:**
- Consumes: `parse(args: &[String]) -> Result<Option<NaturalIntent>>`.
- Produces these new `NaturalIntent` variants:

```rust
CurrentSession,
ListSessions,
ShowSessionRecent { name: String, limit: usize },
ClearSession { name: Option<String> },
RunInSession { name: String, task: String },
```

- [ ] **Step 1: Add failing parser tests for the approved Chinese and English forms**

Extend `parses_supported_intents_without_matching_normal_tasks` with:

```rust
assert_eq!(parse_text("查看当前会话"), Some(NaturalIntent::CurrentSession));
assert_eq!(parse_text("show current session"), Some(NaturalIntent::CurrentSession));
assert_eq!(parse_text("列出所有会话"), Some(NaturalIntent::ListSessions));
assert_eq!(parse_text("list sessions"), Some(NaturalIntent::ListSessions));
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
```

Extend `rejects_incomplete_or_invalid_intents` with:

```rust
assert!(parse(&["查看 dev 最近 0 条对话".to_string()]).is_err());
assert!(parse(&["清空 会话".to_string()]).is_err());
assert!(parse(&["在 dev 会话中".to_string()]).is_err());
assert!(parse(&["in session dev".to_string()]).is_err());
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
TOOLCHAIN="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" test intent_cmd::tests -- --nocapture
```

Expected: compilation fails because the new enum variants do not exist.

- [ ] **Step 3: Add the minimal variants and exact parser branches**

Add the variants to `NaturalIntent` exactly as defined above. In `parse`, preserve the existing search/context branches and add session matching before returning `Ok(None)`.

Use exact forms for current/list/clear-current:

```rust
if matches_ignore_ascii_case(text, &["查看当前会话", "show current session"]) {
    return Ok(Some(NaturalIntent::CurrentSession));
}
if matches_ignore_ascii_case(text, &["列出所有会话", "列出会话", "list sessions"]) {
    return Ok(Some(NaturalIntent::ListSessions));
}
if matches_ignore_ascii_case(text, &["清空当前会话", "clear current session"]) {
    return Ok(Some(NaturalIntent::ClearSession { name: None }));
}
```

For Chinese named history, require `查看 `, ` 最近 `, and one of ` 条对话`, ` 条消息`, or ` 条上下文`. Parse the middle number with the existing `parse_limit` and reject an empty name.

For English named history, require `show last `, ` messages in session `, a positive integer, and a non-empty single-token session name.

For named clear, require `清空 <name> 会话` or `clear session <name>` and reject an empty name.

For named-session tasks, require `在 <name> 会话中 <task>` or `in session <name> <task>`. Session names are the non-empty segment before ` 会话中 ` in Chinese and the first token after `in session ` in English. Reject an empty task.

Do not add fuzzy matching, regular-expression dependencies, persistent state, or a second session-name validator.

- [ ] **Step 4: Run the focused tests and verify they pass**

Run:

```bash
TOOLCHAIN="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" fmt --check
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" test intent_cmd::tests -- --nocapture
```

Expected: all intent parser tests pass, including existing search/context cases.

- [ ] **Step 5: Commit the parser**

```bash
git add src/intent_cmd.rs
git commit -m "feat: parse natural-language session intents"
```

---

### Task 2: Route session intents through existing behavior

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes the new `NaturalIntent` variants from Task 1.
- Reuses `session_cmd::run_session_command`, `Cli.session`, `Cli.empty_session`, `default_session_name`, and `run`.
- Produces no new public abstraction or persistent state.

- [ ] **Step 1: Extend the pre-config read-only router**

Add branches to `run_pre_config_intent`:

```rust
Some(NaturalIntent::CurrentSession) => {
    Ok(Some(session_cmd::run_session_command(&[])?))
}
Some(NaturalIntent::ListSessions) => {
    let args = vec!["list".to_string()];
    Ok(Some(session_cmd::run_session_command(&args)?))
}
Some(NaturalIntent::ShowSessionRecent { name, limit }) => {
    let args = vec![
        "show".to_string(),
        name.clone(),
        "--limit".to_string(),
        limit.to_string(),
    ];
    Ok(Some(session_cmd::run_session_command(&args)?))
}
```

Keep save-last-search and current daily-context behavior unchanged.

- [ ] **Step 2: Translate clear and named-session task intents before config initialization**

Change `let cli = Cli::parse();` to `let mut cli = Cli::parse();` and calculate the effective text after pre-config intent routing:

```rust
let text = match natural_intent.as_ref() {
    Some(NaturalIntent::ClearSession { name }) => {
        cli.session = name.clone().map(Some);
        cli.empty_session = true;
        None
    }
    Some(NaturalIntent::RunInSession { name, task }) => {
        cli.session = Some(Some(name.clone()));
        Some(task.clone())
    }
    _ => cli.text()?,
};
```

For `ClearSession { name: None }`, leave `cli.session` unset so `run` resolves the Beijing-date daily session. For `Some(name)`, set `cli.session = Some(Some(name))`.

Remove the original later `let text = cli.text()?;` line so stdin/text is read only once.

- [ ] **Step 3: Ensure only the search-based execution intent uses `run_builtin_intent`**

Keep `run_builtin_intent` responsible only for `DoFromLastSearch`. `RunInSession` must enter `run` with the translated `cli.session` and task so it uses the exact existing named-session context path.

- [ ] **Step 4: Run focused and full tests**

Run:

```bash
TOOLCHAIN="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" fmt --check
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" test intent_cmd::tests
PATH="$TOOLCHAIN:$PATH" "$TOOLCHAIN/cargo" test
```

Expected: all tests pass; no model/client tests regress.

- [ ] **Step 5: Commit routing**

```bash
git add src/main.rs
git commit -m "feat: route natural-language session operations"
```

---

### Task 3: Document and verify the complete behavior

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/aicmd-usage.md`
- Modify: `docs/superpowers/plans/2026-06-19-command-simplification-plan.md`

**Interfaces:**
- Documents the exact parser forms from Task 1 and routing semantics from Task 2.
- Does not introduce additional aliases beyond tested forms.

- [ ] **Step 1: Add concise Chinese and English examples**

Add these examples near the existing session section in `README.md`:

```bash
aicmd 查看当前会话
aicmd 列出所有会话
aicmd 查看 dev 最近 5 条对话
aicmd 在 dev 会话中继续处理刚才的问题
aicmd 清空 dev 会话
```

Add the matching forms to `README.en.md`:

```bash
aicmd show current session
aicmd list sessions
aicmd show last 5 messages in session dev
aicmd in session dev continue with the previous task
aicmd clear session dev
```

State in both files that named-session use affects only the current invocation and later plain commands return to the daily session. State that clearing always asks for confirmation.

- [ ] **Step 2: Update detailed usage and the earlier simplification plan**

Add the full supported form table and behavior boundaries to `docs/aicmd-usage.md`. In `docs/superpowers/plans/2026-06-19-command-simplification-plan.md`, mark Phase 4 complete and state that persistent switching remains intentionally unsupported.

- [ ] **Step 3: Review documentation with `karpathy-guidelines`**

Check that documentation:

- leads with user actions rather than implementation details;
- contains equivalent Chinese and English examples;
- does not imply persistent switching;
- does not add unimplemented aliases;
- keeps advanced explicit commands available.

Fix any discrepancy before continuing.

- [ ] **Step 4: Run manual tests in an isolated session directory**

Use a temporary `AICMD_SESSIONS_DIR` containing `cmd-<today>.yaml` and `dev.yaml`, then run:

```bash
AICMD_SESSIONS_DIR="$TEST_SESSIONS" target/debug/aicmd 查看当前会话
AICMD_SESSIONS_DIR="$TEST_SESSIONS" target/debug/aicmd 列出所有会话
AICMD_SESSIONS_DIR="$TEST_SESSIONS" target/debug/aicmd 查看 dev 最近 2 条对话
AICMD_SESSIONS_DIR="$TEST_SESSIONS" target/debug/aicmd show last 2 messages in session dev
```

Expected: current resolves to today's Beijing-date name, list includes both fixtures, and each show command prints exactly two non-system messages.

Run the clear-current and clear-named commands in a PTY, answer `n`, then compare the fixture files to their originals.

Expected: each prompt displays the exact resolved session name and both files remain byte-for-byte unchanged after cancellation.

Run `aicmd in session new-session output hello`, approve only the generated safe command, decline summary, then verify `new-session.yaml` exists. Run a later plain task and verify its session note is written to the daily session rather than `new-session.yaml`.

- [ ] **Step 5: Run full verification**

Run:

```bash
TOOLCHAIN="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
export PATH="$TOOLCHAIN:$PATH"
"$TOOLCHAIN/cargo" fmt --check
"$TOOLCHAIN/cargo" test
"$TOOLCHAIN/cargo" clippy --all --all-targets -- -D warnings
"$TOOLCHAIN/cargo" build --release
git diff --check
```

Expected: all commands exit `0` with no test failures, warnings, formatting differences, or whitespace errors.

- [ ] **Step 6: Commit documentation**

```bash
git add README.md README.en.md docs/aicmd-usage.md docs/superpowers/plans/2026-06-19-command-simplification-plan.md
git commit -m "docs: explain natural-language session operations"
```

- [ ] **Step 7: Install and verify the local binary after integration**

After the feature branch is locally merged and the merged release build passes:

```bash
install -m 0755 target/release/aicmd "$HOME/.local/bin/aicmd"
"$HOME/.local/bin/aicmd" --version
```

Expected: installed binary reports the repository version and the natural-language session smoke tests work from `~/.local/bin/aicmd`.
