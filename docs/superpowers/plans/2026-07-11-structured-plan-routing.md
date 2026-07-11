# AICmd Structured Plan Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let plain `aicmd <task>` select direct command execution, script work, MCP search, or diagnosis through a strict validated JSON plan.

**Architecture:** Add a small `plan_cmd` module as the only model-output parser. Move the current interactive path out of `main.rs` into plan, confirm, execute, and result modules. Existing `do`, `search`, and `err` keep their explicit behavior and become advanced entry points in help and documentation.

**Tech Stack:** Rust 2021, existing `serde`/`serde_json`, existing AICmd `Input`, `Role`, MCP, command-cache, repair, and terminal helpers. Do not add dependencies.

## Global Constraints / 全局约束

- The structured plan has exactly four modes: `direct`, `script`, `search`, `diagnose`.
- Use `#[serde(deny_unknown_fields)]`; invalid JSON, unknown fields, missing required fields, or non-JSON surrounding text are safe failures, never heuristic fallbacks.
- Keep the existing high-risk second confirmation, session recording, command cache, optional AI summary, and two-attempt repair limit.
- Keep `aicmd do`, `aicmd search`, and `aicmd err` compatible; do not add a TUI or generic agent loop.
- Remove only command-parsing cleanup heuristics. Retain terminal rendering for search output and summaries.
- Documentation is bilingual. Do not stage local files such as `mcp.json`, `.aicmd/`, `.DS_Store`, or `tavily_key.txt`.

---

## File Structure / 文件结构

| File | Responsibility |
| --- | --- |
| `src/plan_cmd.rs` | Strict JSON plan type, validation, planner request, and planner model call. |
| `src/confirm_cmd.rs` | Command display, risk display, `Run? [Y/n/?]`, and advanced selection. |
| `src/execute_cmd.rs` | Shell execution, output streaming/decoding, and cwd capture wrapper. |
| `src/result_cmd.rs` | Summary, session note, cache write, and failure repair follow-up. |
| `src/main.rs` | Startup, explicit command shortcuts, plan routing, and compatibility adapters only. |
| `assets/roles/%shell%.md` | Strict JSON-only planner prompt. |
| `src/help_cmd.rs`, `README.md`, `README.en.md`, `docs/aicmd-usage.md` | Present automatic routing as ordinary usage; retain explicit modes as advanced usage. |

---

### Task 1: Add strict structured planning / 增加严格结构化计划

**Files:**
- Create: `src/plan_cmd.rs`
- Modify: `src/main.rs`
- Modify: `assets/roles/%shell%.md`

**Interfaces:**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanMode { Direct, Script, Search, Diagnose }

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub mode: PlanMode,
    #[serde(default)] pub command: String,
    #[serde(default)] pub query: String,
    #[serde(default)] pub problem: String,
}

pub fn parse_execution_plan(raw: &str) -> Result<ExecutionPlan>;
pub async fn request_execution_plan(
    config: &GlobalConfig, input: &Input, abort_signal: AbortSignal,
) -> Result<ExecutionPlan>;
```

- [ ] **Step 1: Write parser tests before implementation**

Add tests in `src/plan_cmd.rs` that assert:

```rust
assert_eq!(
    parse_execution_plan(r#"{"mode":"direct","command":"pwd"}"#)?.mode,
    PlanMode::Direct,
);
assert!(parse_execution_plan("```json\n{\"mode\":\"direct\",\"command\":\"pwd\"}\n```").is_err());
assert!(parse_execution_plan(r#"{"mode":"direct","command":"","query":"x"}"#).is_err());
assert!(parse_execution_plan(r#"{"mode":"search","query":"rust","extra":true}"#).is_err());
```

- [ ] **Step 2: Run the parser tests and confirm they fail**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests
```

Expected: compilation failure because `plan_cmd` and `parse_execution_plan` do not exist.

- [ ] **Step 3: Implement exact parsing and semantic validation**

Implement `parse_execution_plan` with `serde_json::from_str`, reject whitespace-only required fields, and reject values that belong to another mode:

```rust
match plan.mode {
    PlanMode::Direct | PlanMode::Script if plan.command.trim().is_empty() => bail!("Invalid execution plan: '{}' requires a non-empty command", plan.mode),
    PlanMode::Search if plan.query.trim().is_empty() => bail!("Invalid execution plan: 'search' requires a non-empty query"),
    PlanMode::Diagnose if plan.problem.trim().is_empty() => bail!("Invalid execution plan: 'diagnose' requires a non-empty problem"),
    _ => Ok(plan),
}
```

The planner request uses `SHELL_ROLE`, calls the existing non-streaming `call_chat_completions`, and returns only the parsed plan. Do not call `after_chat_completion` with raw JSON.

- [ ] **Step 4: Replace the shell role output contract**

Rewrite `assets/roles/%shell%.md` so it instructs the model to emit only one JSON object matching the four-mode contract. Include direct/script/search/diagnose examples, state that Markdown fences and explanatory text are invalid, and preserve shell, macOS/Linux, safety, and bilingual requirements.

- [ ] **Step 5: Run focused tests**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests
```

Expected: all new parser tests pass.

- [ ] **Step 6: Commit the isolated planner**

```bash
git add src/plan_cmd.rs src/main.rs 'assets/roles/%shell%.md'
git commit -m "feat: add structured execution plans"
```

---

### Task 2: Route automatic plans while preserving explicit commands / 路由自动计划并保留显式命令

**Files:**
- Modify: `src/main.rs`
- Modify: `src/plan_cmd.rs`
- Test: `src/plan_cmd.rs`

**Interfaces:**

```rust
async fn route_execution_plan(
    config: &GlobalConfig,
    shell: &Shell,
    input: Input,
    plan: ExecutionPlan,
    abort_signal: AbortSignal,
    cache_task: Option<String>,
) -> Result<()>;
```

- [ ] **Step 1: Write route selection tests with an injected plan**

Extract a pure helper that maps a plan to a route enum and test all values:

```rust
assert_eq!(route_kind(&PlanMode::Direct), RouteKind::Command);
assert_eq!(route_kind(&PlanMode::Script), RouteKind::Command);
assert_eq!(route_kind(&PlanMode::Search), RouteKind::Search);
assert_eq!(route_kind(&PlanMode::Diagnose), RouteKind::Diagnose);
```

- [ ] **Step 2: Run the focused tests and confirm they fail**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests
```

Expected: failure because `RouteKind` and `route_kind` do not exist.

- [ ] **Step 3: Implement routing adapters**

Implement `route_execution_plan` with these exact behaviors:

```text
direct  -> shared command confirmation/execution flow with plan.command
script  -> shared command confirmation/execution flow with plan.command
search  -> call_mcp_raw("search", plan.query), summarize_mcp_output(...), prompt_search_follow_up(...)
diagnose -> build an Input from plan.problem and enter the existing repair/command-diagnosis generation flow without executing a shell command automatically
```

For `--dry-run`, print the validated plan JSON rather than sending it to execution/MCP. For `--print`, print `command` for direct/script; print the planned mode and query/problem for search/diagnose. Invalid plans return a bilingual error before any route is invoked.

- [ ] **Step 4: Change the normal entry path only**

In `run`, replace the normal `shell_execute` model-generation call with:

```rust
let plan = request_execution_plan(&config, &input, abort_signal.clone()).await?;
route_execution_plan(&config, &SHELL, input, plan, abort_signal, cache_task).await?;
```

Do not route calls that arrived through explicit `do`, `search`, or `err`; they retain their current purpose and compatibility behavior.

- [ ] **Step 5: Run route tests and existing module tests**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests
PATH="$HOME/.cargo/bin:$PATH" cargo test do_cmd::tests search_cmd::tests repair_cmd::tests
```

Expected: direct/script/search/diagnose route mapping passes; explicit-command tests remain green.

- [ ] **Step 6: Commit routing**

```bash
git add src/main.rs src/plan_cmd.rs
git commit -m "feat: route natural language tasks by plan"
```

---

### Task 3: Extract confirmation and execution / 拆分确认与执行

**Files:**
- Create: `src/confirm_cmd.rs`
- Create: `src/execute_cmd.rs`
- Modify: `src/main.rs`

**Interfaces:**

```rust
pub enum ConfirmationAction { Execute, Revise, Describe, Copy, Regenerate, Quit }
pub fn confirm_command(command: &str, from_cache: bool) -> Result<ConfirmationAction>;

pub struct CommandOutput { pub code: i32, pub stdout: String, pub stderr: String }
pub fn run_command(shell: &Shell, command: &str) -> Result<CommandOutput>;
pub fn with_cwd_capture(shell: &Shell, command: &str) -> String;
```

- [ ] **Step 1: Write pure confirmation and execution tests**

Test the stable, non-terminal helpers:

```rust
assert!(classify_command_risk("rm -rf /tmp/x").requires_confirmation());
assert_eq!(with_cwd_capture(&shell, "pwd"), "pwd"); // when AICMD_CWD_FILE is unset
assert_eq!(CommandOutput { code: 0, stdout: "ok".into(), stderr: String::new() }.code, 0);
```

Keep `read_single_key` terminal interaction thin; do not build a fake TUI test framework.

- [ ] **Step 2: Move confirmation behavior without changing semantics**

Move command color/risk display, `Run? [Y/n/?]`, advanced revise/describe/copy/quit actions, cached `g` regenerate, and high-risk second confirmation from `handle_generated_command` into `confirm_cmd`.

The function returns an action; it must not run a command.

- [ ] **Step 3: Move shell process behavior without changing semantics**

Move `command_with_cwd_capture`, `run_shell_command_capture`, `stream_and_capture`, `write_console_chunk`, and `decode_command_output` into `execute_cmd`. Keep platform `cfg` branches exactly equivalent.

- [ ] **Step 4: Compile and test after the extraction**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
```

Expected: all tests pass; direct shell execution behavior is unchanged apart from receiving a validated plan command.

- [ ] **Step 5: Commit execution-chain extraction**

```bash
git add src/confirm_cmd.rs src/execute_cmd.rs src/main.rs
git commit -m "refactor: split command confirmation and execution"
```

---

### Task 4: Extract results and remove command-cleanup heuristics / 拆分结果处理并删除命令清洗启发式

**Files:**
- Create: `src/result_cmd.rs`
- Modify: `src/main.rs`
- Modify: `src/repair_cmd.rs`
- Delete from: `src/main.rs` (`sanitize_generated_command`, `remove_markdown_and_prose_from_command`, `keep_generated_command_line`, `contains_cjk`, `is_shell_line_that_may_contain_cjk`, `remove_leading_missing_target_exit_guard`, install/precheck rewrite helpers, and their tests)

**Interfaces:**

```rust
pub async fn finalize_command_result(
    config: &GlobalConfig,
    shell: &Shell,
    user_task: &str,
    command: &str,
    output: &CommandOutput,
    abort_signal: AbortSignal,
    cache_task: Option<&str>,
    repair_attempts: u8,
) -> Result<ResultAction>;
```

`ResultAction` is either `Done(i32)` or `Repair { prompt: String, attempts: u8 }`.

- [ ] **Step 1: Move existing output-result tests first**

Move or recreate these assertions in `src/result_cmd.rs` before deleting code:

```rust
let note = build_execution_session_note("printf hello", 0, "hello", "", Some("printed hello"));
assert!(note.contains("Exit code: 0"));
assert!(note.contains("STDOUT:\nhello"));
```

Add one test that a zero exit code selects cache recording and a non-zero exit code with fewer than two attempts returns `ResultAction::Repair`.

- [ ] **Step 2: Move result responsibilities**

Move command summary, session-note construction, session append, successful-command cache writes, failure `fix/explain/copy/quit`, and repair prompt construction to `result_cmd`. Keep optional AI summary behavior and the repair limit exactly as today.

- [ ] **Step 3: Delete the obsolete sanitization chain**

Delete the listed sanitizer functions and all tests asserting:

```text
provider marker stripping
Markdown fence removal
natural-language line filtering
Windows find argument rewriting
install precheck removal
```

Do not delete `clean_terminal_markdown`, `strip_ansi_codes`, `classify_command_risk`, shell quoting, or output decoding.

- [ ] **Step 4: Make strict plan parsing the sole command boundary**

The direct/script route passes `plan.command.trim()` unchanged into confirmation/execution. If the model returns Markdown or prose instead of valid JSON, parsing fails before this point.

- [ ] **Step 5: Run the full validation suite**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo build
git diff --check
```

Expected: no sanitizer symbol remains in `src/main.rs`; all tests pass.

- [ ] **Step 6: Commit the cleanup**

```bash
git add src/result_cmd.rs src/repair_cmd.rs src/main.rs
git commit -m "refactor: separate command results from planning"
```

---

### Task 5: Reframe help and docs around automatic routing / 围绕自动路由更新帮助与文档

**Files:**
- Modify: `src/help_cmd.rs`
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/aicmd-usage.md`

- [ ] **Step 1: Update the primary examples**

Use the following ordinary examples in both languages:

```bash
aicmd 当前目录有多少文件
aicmd "读取 data/orders.csv，按用户统计订单金额，输出到 output/user_totals.csv"
aicmd "查一下 Docker 最新安装方式"
aicmd "分析这个报错：permission denied"
```

State that AICmd chooses command, script, search, or diagnosis automatically.

- [ ] **Step 2: Move explicit modes to advanced usage**

Keep these examples under an advanced heading:

```bash
aicmd do "复杂文件处理任务"
aicmd search "指定 MCP 查询"
aicmd err -- <command> [args...]
```

Explain that they force an explicit workflow and remain useful for users who need control.

- [ ] **Step 3: Document invalid-plan behavior accurately**

Add a concise bilingual note:

```text
If the model does not return a valid structured plan, AICmd stops safely and asks you to retry. It never guesses a shell command from Markdown or prose.
如果模型没有返回有效结构化计划，AICmd 会安全停止并提示重试；不会从 Markdown 或自然语言中猜测 shell 命令。
```

- [ ] **Step 4: Review docs with karpathy-guidelines**

Check that the docs only describe implemented behavior, keep `aicmd <task>` first, avoid duplicating the full command reference, and preserve matching Chinese/English meanings.

- [ ] **Step 5: Verify rendered help and documentation diffs**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" target/debug/aicmd help
PATH="$HOME/.cargo/bin:$PATH" target/debug/aicmd help do
git diff --check
```

Expected: help clearly states that plain tasks are auto-routed; advanced commands remain discoverable.

- [ ] **Step 6: Commit docs**

```bash
git add src/help_cmd.rs README.md README.en.md docs/aicmd-usage.md
git commit -m "docs: explain automatic task routing"
```

---

### Task 6: End-to-end verification and local installation / 端到端验证与本地安装

**Files:**
- No source changes expected unless verification exposes a defect.

- [ ] **Step 1: Verify compilation and tests**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo build --release
git diff --check
```

- [ ] **Step 2: Verify strict invalid output without a live model**

Run the plan parser tests and confirm an input containing fenced JSON fails. Verify no command/MCP helper is called by the route tests.

- [ ] **Step 3: Perform live smoke tests only with configured local services**

Run one request per route:

```bash
target/release/aicmd 当前目录有多少文件
target/release/aicmd "读取 data/orders.csv，按用户统计订单金额，输出到 output/user_totals.csv"
target/release/aicmd "查一下 Docker 最新安装方式"
target/release/aicmd "分析这个报错：command not found"
```

For direct/script, stop at confirmation unless the generated operation is read-only. Confirm search and diagnosis do not auto-execute shell changes.

- [ ] **Step 4: Install the verified local release binary**

Run:

```bash
install -m 0755 target/release/aicmd "$HOME/.local/bin/aicmd"
"$HOME/.local/bin/aicmd" help
```

Expected: installed help displays automatic task routing text.

- [ ] **Step 5: Record the implementation and report commit state**

Save the final routing contract, removed heuristics, affected files, and verification results to agentmemory. Report the local commits and leave unrelated local files unstaged. Do not push GitHub unless the user explicitly requests it.

---

## Plan Self-Review / 计划自审

- **Spec coverage:** Tasks 1–2 implement strict structured planning and all four automatic routes. Tasks 3–4 split the execution chain and remove only command parsing heuristics. Task 5 changes the ordinary/advanced command presentation. Task 6 verifies and installs the local binary.
- **Scope:** No new dependencies, providers, TUI, generic agent loop, or configuration format changes are introduced.
- **Type consistency:** `ExecutionPlan` is created and validated in Task 1, routed in Task 2, and only direct/script command strings reach Tasks 3–4. `CommandOutput` is created in Task 3 and consumed in Task 4.
- **Safety:** Invalid plans stop before side effects; high-risk confirmation and repair limits remain intact.
