# AICmd P0-P2 Interaction, Recovery, and Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a no-argument continuous prompt, session-based failed-task continuation, evidence-gated search execution, Git change recovery guidance, and actionable MCP diagnostics without adding user-facing commands or dependencies.

**Architecture:** Keep the existing single-request execution path authoritative. P0 adds a small parent prompt that invokes the current executable with the daily session for each line. P1 extends deterministic intent routing and the existing search context loader. P2 adds isolated helpers for Git status deltas and MCP configuration/runtime diagnostics, then reuses them from the current execution and doctor paths.

**Tech Stack:** Rust 2021, Clap, existing inquire/is-terminal/serde dependencies, std::process, existing session/search/MCP modules.

## Global Constraints

- Do not add a dependency, persistent default session, configuration field, or user-facing command.
- Existing explicit commands and text invocations must remain compatible.
- The continuous prompt runs only when no text/options are supplied and stdin/stdout are terminals.
- Every interactive task uses the Beijing-date `cmd-YYYYMMDD` session with context enabled.
- Search evidence validation is local and must occur before model invocation.
- Git change reporting is advisory and must never block execution.
- MCP doctor checks are local and must not start a server or print environment values.
- Windows remains unsupported.
- Documentation is Chinese and English and must receive `karpathy-guidelines` review.
- Do not stage `mcp.json`, `.env`, `.aicmd/`, `.DS_Store`, `.codebase-memory/`, or `tavily_key.txt`.

---

### Task 1: Add the P0 continuous prompt

**Files:**
- Create: `src/interactive_cmd.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produce: `interactive_cmd::should_start(cli: &Cli) -> bool`
- Produce: `interactive_cmd::is_exit_input(input: &str) -> bool`
- Produce: `interactive_cmd::child_args(session: &str, input: &str) -> Vec<String>`
- Produce: `interactive_cmd::run(session: &str) -> Result<i32>`
- Consume: existing `default_session_name()` and current executable.

- [ ] **Step 1: Add focused tests**

Test that:

```rust
assert!(is_exit_input("exit"));
assert!(is_exit_input(" quit "));
assert!(is_exit_input(".exit"));
assert!(!is_exit_input("exit vim"));
assert_eq!(
    child_args("cmd-20260712", "查看内存"),
    vec!["-s", "cmd-20260712", "查看内存"]
);
```

Add `should_start` tests proving that text, `--dry-run`, `--print`, model/session/file/summary/cache/list/empty options suppress the prompt.

- [ ] **Step 2: Implement the minimal parent loop**

Use `std::io::Write` to print `AICmd> ` and `read_line` for input. Resolve `env::current_exe()` once. For each non-empty non-exit line:

```rust
Command::new(&exe)
    .args(child_args(session, line))
    .status()
```

Ignore child non-zero status and continue. Return an error only when the child cannot start. EOF returns `0`.

- [ ] **Step 3: Route before config initialization**

In `main`, after parsing `Cli` and before natural-intent/config work:

```rust
if interactive_cmd::should_start(&cli) {
    process::exit(interactive_cmd::run(&default_session_name())?);
}
```

Do not enter the prompt for piped stdin.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --check
cargo test interactive_cmd::tests
cargo test
```

Commit:

```bash
git add src/interactive_cmd.rs src/main.rs
git commit -m "feat: add continuous terminal prompt"
```

---

### Task 2: Add failed-task continuation and search evidence validation

**Files:**
- Modify: `src/intent_cmd.rs`
- Modify: `src/main.rs`
- Modify: `src/search_cmd.rs`
- Modify: `src/do_cmd.rs`

**Interfaces:**
- Add: `NaturalIntent::ContinueLastFailure`
- Produce: `search_cmd::validate_execution_evidence(raw: &RawSearchRecord) -> Result<()>`
- Change: `do_cmd::resolve_context_files` to include both summary and raw paths for every search reference.

- [ ] **Step 1: Add parser and routing tests**

Add exact forms:

```text
继续修复刚才失败的任务
继续处理刚才失败的任务
continue fixing the last failed task
continue the last failed task
```

Route this intent by setting:

```rust
cli.session = Some(Some(default_session_name()));
text = original normalized continuation text;
```

This must enable existing context and still use the normal planner.

- [ ] **Step 2: Add evidence tests**

Use raw records containing:

- URL plus `brew install demo`: pass.
- URL without a recognized command line: fail.
- recognized command without URL: fail.
- neither: fail.

Error text must say the saved search is insufficient and recommend a more specific search.

- [ ] **Step 3: Implement evidence validation**

Accept URLs by `raw_output.contains("http://") || raw_output.contains("https://")`.

Recognize command lines after trimming optional Markdown list markers and one leading `$ `. Match the exact command prefixes from the design spec. Do not add regex or a dependency.

- [ ] **Step 4: Include raw and summary search files**

For every `--from-search <name>`:

1. require summary path;
2. load raw record;
3. validate it;
4. include summary path and raw path in `read_file_context`.

Update the search-based prompt:

```text
Do not invent, complete, or repair source URLs or installation commands from model memory.
只允许使用搜索证据中完整出现的来源 URL 和安装命令，不得依靠模型记忆补全。
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --check
cargo test intent_cmd::tests
cargo test search_cmd::tests
cargo test do_cmd::tests
cargo test
```

Commit:

```bash
git add src/intent_cmd.rs src/main.rs src/search_cmd.rs src/do_cmd.rs
git commit -m "feat: validate search execution evidence"
```

---

### Task 3: Report Git-visible changes after modifying commands

**Files:**
- Create: `src/change_report_cmd.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produce: `GitSnapshot::capture(cwd: &Path) -> Option<GitSnapshot>`
- Produce: `GitSnapshot::changes_since(&self, after: &GitSnapshot) -> Vec<String>`
- Produce: `format_recovery_report(changes: &[String]) -> String`
- Consume: existing `CommandRiskLevel`.

- [ ] **Step 1: Add delta tests**

Parse porcelain lines as complete status records keyed by their displayed path. Verify:

```rust
before = [" M existing.txt"];
after = [" M existing.txt", "?? new.txt"];
changes == ["?? new.txt"];
```

Also verify a changed status for the same path is reported and unchanged lines are omitted.

- [ ] **Step 2: Implement snapshot capture**

Run:

```text
git status --porcelain=v1 --untracked-files=all
```

with `current_dir(cwd)`. Return `None` for non-Git directories, non-zero status, invalid UTF-8, or spawn failure. Do not print errors.

- [ ] **Step 3: Integrate around execution**

Capture `before` immediately before `run_command_capture` when the existing risk level is `ChangesSystem` or `Destructive`. Capture `after` after command completion, including failed commands. If both exist and the delta is non-empty, print the localized report.

The report must recommend `git diff` and state that AICmd does not automatically reset or delete files.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --check
cargo test change_report_cmd::tests
cargo test
```

Commit:

```bash
git add src/change_report_cmd.rs src/main.rs
git commit -m "feat: report file changes after execution"
```

---

### Task 4: Add MCP configuration and runtime diagnostics

**Files:**
- Modify: `src/mcp_cmd.rs`
- Modify: `src/doctor_cmd.rs`

**Interfaces:**
- Produce: `mcp_cmd::diagnose_config() -> Vec<McpDiagnostic>`
- Produce public read-only fields or formatting methods needed by `doctor_cmd`.
- Produce: `mcp_stage_error(stage, server, detail, suggestion) -> anyhow::Error`.

- [ ] **Step 1: Add configuration diagnostic tests**

Use in-memory JSON values and cover:

- command references missing server;
- unsupported server type;
- missing or empty server command;
- absolute executable missing;
- PATH executable missing;
- valid server/command mapping.

Diagnostics must not include values from the server `env` object.

- [ ] **Step 2: Implement local configuration checks**

Reuse existing `mcp_root` and `mcp_commands`. Add a pure `diagnose_value(&Value)` helper plus a file-loading wrapper. Resolve executables with:

- absolute/path-containing command: `Path::is_file`;
- bare command: scan `PATH` directories.

Expose only name, status, detail, and suggestion.

- [ ] **Step 3: Integrate with doctor**

Replace the current coarse MCP check with one summary check per diagnostic. Keep `doctor` exit behavior unchanged. Do not start MCP servers.

- [ ] **Step 4: Label runtime failures**

Wrap errors at:

- spawn: `start`;
- initialize response: `initialize`;
- tools list response: `tools/list`;
- `choose_tool`: `tool selection`;
- call response: `tools/call`.

Timeout suggestions reference the existing `AICMD_MCP_START_TIMEOUT_SECS` or `AICMD_MCP_CALL_TIMEOUT_SECS`. Other failures recommend `aicmd doctor`. Preserve the original error as the source.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --check
cargo test mcp_cmd::tests
cargo test doctor_cmd::tests
cargo test
```

Commit:

```bash
git add src/mcp_cmd.rs src/doctor_cmd.rs
git commit -m "feat: add actionable MCP diagnostics"
```

---

### Task 5: Documentation, review, and installed-binary verification

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/aicmd-usage.md`

- [ ] **Step 1: Update user documentation**

Document:

- `aicmd` continuous prompt and exit forms;
- daily-session context behavior;
- continuing a failed task;
- search evidence rejection behavior;
- Git change/recovery output;
- expanded `doctor` and MCP stage errors.

Keep explicit subcommands in advanced reference. Do not add commands that do not exist.

- [ ] **Step 2: Review with karpathy-guidelines**

Confirm:

- Chinese and English describe the same behavior;
- the default path appears before advanced options;
- no future claims or unsupported rollback behavior;
- no repeated command tables.

- [ ] **Step 3: Run complete verification**

```bash
TOOLCHAIN="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
export PATH="$TOOLCHAIN:$PATH"
"$TOOLCHAIN/cargo" fmt --check
"$TOOLCHAIN/cargo" test
"$TOOLCHAIN/cargo" clippy --all --all-targets -- -D warnings
"$TOOLCHAIN/cargo" build --release
git diff --check
```

- [ ] **Step 4: Run manual smoke tests**

Use temporary session/search/MCP directories and verify:

- no-argument PTY prompt accepts two tasks and exits;
- child failure returns to `AICmd>`;
- failed-task continuation uses the daily named session;
- weak saved search fails before model generation;
- strong saved search reaches dry-run generation;
- a controlled Git file change prints only new/changed status lines;
- `doctor` reports missing MCP executable;
- a controlled MCP timeout names its stage.

- [ ] **Step 5: Commit documentation**

```bash
git add README.md README.en.md docs/aicmd-usage.md
git commit -m "docs: explain interactive recovery workflow"
```

- [ ] **Step 6: Merge, install, and verify**

After final review, merge locally into `main`, rebuild, install:

```bash
install -m 0755 target/release/aicmd "$HOME/.local/bin/aicmd"
```

Verify `aicmd --version`, `aicmd doctor`, and an installed-binary PTY prompt. Preserve unrelated local files and do not push GitHub unless explicitly requested.
