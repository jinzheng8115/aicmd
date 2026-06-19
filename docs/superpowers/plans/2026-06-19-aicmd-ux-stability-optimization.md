# AICmd UX Stability Optimization Implementation Plan
# AICmd 用户体验与稳定性优化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **给智能体执行者：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，按任务逐项实现。本计划使用 checkbox 追踪进度。

**Goal:** Make AICmd easier to use and more predictable by reducing command-memory burden, exposing current configuration clearly, reusing successful commands, and adding a safe failure-repair loop.

**目标：** 通过减少用户需要记忆的命令、清晰展示当前配置、复用成功命令、增加安全的失败修复流程，让 AICmd 更易用、更稳定、更可预期。

**Architecture:** Keep the existing Rust CLI structure. Add small focused modules for command history/cache and failure repair, extend existing config/doctor paths surgically, and update bilingual docs without changing the product scope. Do not reintroduce native Windows support.

**架构：** 保持现有 Rust CLI 结构。新增小而专注的命令历史/缓存与失败修复模块，谨慎扩展现有 config/doctor 路径，并同步更新中英文文档。不重新引入 Windows 原生支持。

**Tech Stack:** Rust, Clap, serde/serde_yaml, existing AICmd config/session/model client modules, Markdown documentation.

**技术栈：** Rust、Clap、serde/serde_yaml、现有 AICmd config/session/model client 模块、Markdown 文档。

---

## Assumptions / 假设

- Repository path: `/Volumes/Code/open_source/aicmd`.
- Current public positioning remains: natural-language terminal command runner for macOS/Linux; Windows users should use WSL.
- Sensitive/local files must not be committed: `mcp.json`, `.env`, `tavily_key.txt`, `.aicmd/`, `.DS_Store`.
- Existing release `v0.4.0` does not include the latest `temperature=0` main-branch commit. A new release should be `v0.4.1` only after implementation and tests pass.
- Documentation must stay bilingual. `README.md` remains Chinese default and links to `README.en.md`.

## Success Criteria / 成功标准

- A user can run `aicmd config status` and see the active model, config path, temperature, AI summary status, MCP/search status, and current session without exposing API keys.
- Repeating the same successful natural-language command can reuse a prior approved command instead of calling the model every time.
- When execution fails, the user can choose a repair action that sends command + exit code + stdout/stderr to the model and proposes a revised command for confirmation.
- README stays simple: main path first, advanced commands grouped separately.
- `cargo fmt --check`, `cargo test`, `cargo build`, and `git diff --check` pass.

## File Structure / 文件结构

- Modify `/Volumes/Code/open_source/aicmd/src/cli.rs`
  - Add or refine CLI flags/subcommands for `config status`, command reuse options, and repair actions.
- Modify `/Volumes/Code/open_source/aicmd/src/config_cmd.rs`
  - Implement `aicmd config status` and keep existing summary commands.
- Create `/Volumes/Code/open_source/aicmd/src/command_cache.rs`
  - Store and retrieve successful generated commands keyed by normalized task + shell + OS.
- Modify `/Volumes/Code/open_source/aicmd/src/main.rs`
  - Integrate cache lookup before model generation and cache write after successful execution.
- Create `/Volumes/Code/open_source/aicmd/src/repair_cmd.rs`
  - Generate a revised command from failed execution context.
- Modify `/Volumes/Code/open_source/aicmd/src/model_cmd.rs`
  - Add helper for repair prompt if current model dispatch belongs there.
- Modify `/Volumes/Code/open_source/aicmd/assets/roles/%shell%.md`
  - Add bilingual instruction that repeatable, deterministic, valid shell output is preferred.
- Modify `/Volumes/Code/open_source/aicmd/README.md`
  - Keep Chinese README concise; add config status, reuse, repair examples.
- Modify `/Volumes/Code/open_source/aicmd/README.en.md`
  - English counterpart.
- Modify `/Volumes/Code/open_source/aicmd/docs/aicmd-usage.md`
  - Detailed command reference if README would become too long.
- Optionally modify `/Volumes/Code/open_source/aicmd/Cargo.toml` and `/Volumes/Code/open_source/aicmd/Cargo.lock`
  - Only if version bump to `0.4.1` is performed after all changes.

---

## Task 1: Add `aicmd config status` / 增加 `aicmd config status`

**Files:**
- Modify: `/Volumes/Code/open_source/aicmd/src/cli.rs`
- Modify: `/Volumes/Code/open_source/aicmd/src/config_cmd.rs`
- Test by command: `cargo test`, `cargo build`, manual `target/debug/aicmd config status`

- [ ] **Step 1: Inspect current config CLI shape / 查看当前 config 命令结构**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
sed -n '1,240p' src/cli.rs
sed -n '1,260p' src/config_cmd.rs
```

Expected:

```text
Find existing Config subcommand enum and summary on/off/status implementation.
找到现有 Config 子命令枚举，以及 summary on/off/status 的实现位置。
```

- [ ] **Step 2: Add a `status` subcommand to config CLI / 给 config 增加 status 子命令**

Implement the smallest change that lets this parse:

```bash
aicmd config status
```

Expected behavior:

```text
No API keys are printed.
不打印 API key。
```

- [ ] **Step 3: Implement status output / 实现状态输出**

The output should include these fields:

```text
AICmd config status
Config file: <path>
Default model: <client:model or unknown>
Temperature: <value or provider default>
AI summary: on/off
MCP config: configured/missing
Search: configured/missing
Session: <current default session name>
```

Chinese labels can be included after English labels if consistent with current style:

```text
Config file / 配置文件: ~/.aicmd/config.yaml
```

Do not print:

```text
api_key
Authorization header
raw provider credential fields
```

- [ ] **Step 4: Verify manually / 手动验证**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build
./target/debug/aicmd config status
```

Expected:

```text
Build succeeds. config status prints paths and settings without secrets.
构建成功。config status 输出路径和设置，不泄露密钥。
```

- [ ] **Step 5: Commit / 提交**

```bash
git add src/cli.rs src/config_cmd.rs
git commit -m "feat: add config status"
```

---

## Task 2: Add successful command cache / 增加成功命令缓存

**Files:**
- Create: `/Volumes/Code/open_source/aicmd/src/command_cache.rs`
- Modify: `/Volumes/Code/open_source/aicmd/src/main.rs`
- Modify: `/Volumes/Code/open_source/aicmd/src/cli.rs`

- [ ] **Step 1: Define cache data model / 定义缓存数据结构**

Create `/Volumes/Code/open_source/aicmd/src/command_cache.rs` with a simple JSON or YAML file under:

```text
~/.aicmd/command-cache.yaml
```

Each record should contain:

```yaml
- key: "sha256 or stable normalized key"
  task: "目前内存占用率最高的程序"
  shell: "zsh"
  os: "macos"
  command: "ps aux -m | head -1; ..."
  success_count: 1
  last_used_at: "2026-06-19T..."
```

Keep the implementation small:

```rust
pub struct CommandCacheRecord {
    pub key: String,
    pub task: String,
    pub shell: String,
    pub os: String,
    pub command: String,
    pub success_count: u32,
    pub last_used_at: String,
}
```

- [ ] **Step 2: Add lookup and save functions / 增加查询和保存函数**

Expose functions similar to:

```rust
pub fn lookup(task: &str, shell: &str, os: &str) -> Option<CommandCacheRecord>;
pub fn record_success(task: &str, shell: &str, os: &str, command: &str) -> anyhow::Result<()>;
```

Key normalization:

```text
trim whitespace
collapse repeated spaces
lowercase ASCII only
include shell and os in key
```

- [ ] **Step 3: Integrate lookup before model call / 在调用模型前查询缓存**

When user runs a normal command task, before generating a new command, check cache.

If found, show:

```text
Found a previously successful command / 找到一条之前成功执行过的命令:
<command>
reuse(复用) | new(重新生成) | describe(解释) | quit(退出):
```

Behavior:

- `reuse`: use cached command and continue existing execute flow.
- `new`: ignore cache and call model.
- `describe`: explain cached command using existing describe path if available.
- `quit`: exit.

- [ ] **Step 4: Record only successful executions / 只记录成功执行**

After command execution exits with code `0`, save it to cache.

Do not save:

- commands from `--print` if no execution occurred
- failed commands
- commands generated for sensitive-looking tasks containing `password`, `token`, `secret`, `api key`, `密钥`, `密码`

- [ ] **Step 5: Add bypass flag / 增加绕过缓存参数**

Add:

```bash
aicmd --no-cache <task>
```

Expected:

```text
Always call the model and do not offer cached command.
总是调用模型，不提示缓存命令。
```

- [ ] **Step 6: Verify / 验证**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build
./target/debug/aicmd --no-summary 当前目录有多少文件
./target/debug/aicmd --no-summary 当前目录有多少文件
```

Expected second run:

```text
Shows previously successful command prompt.
第二次运行提示可复用之前成功命令。
```

- [ ] **Step 7: Commit / 提交**

```bash
git add src/command_cache.rs src/main.rs src/cli.rs
git commit -m "feat: reuse successful commands"
```

---

## Task 3: Add failure repair loop / 增加执行失败后的修复循环

**Files:**
- Create: `/Volumes/Code/open_source/aicmd/src/repair_cmd.rs`
- Modify: `/Volumes/Code/open_source/aicmd/src/main.rs`
- Modify: `/Volumes/Code/open_source/aicmd/assets/roles/%shell%.md`

- [ ] **Step 1: Capture failure context / 捕获失败上下文**

Ensure the execution path has access to:

```text
original user task
command that failed
exit code
stdout
stderr
shell
os
cwd
```

If stdout/stderr are large, truncate each to the last 4000 characters.

- [ ] **Step 2: Add repair prompt builder / 增加修复提示词构造器**

Create `/Volumes/Code/open_source/aicmd/src/repair_cmd.rs` with a function that builds a bilingual prompt:

```text
You are repairing a failed terminal command for AICmd.
你正在为 AICmd 修复一条执行失败的终端命令。

Rules / 规则:
- Output only one corrected shell command or script wrapper.
- Do not use markdown fences.
- Do not explain outside shell comments or echo/printf.
- Prefer minimal changes from the failed command.
- If the task is impossible or unsafe, output a safe echo command explaining why.

Context / 上下文:
User task / 用户任务: ...
Shell: ...
OS: ...
CWD: ...
Failed command / 失败命令: ...
Exit code / 退出码: ...
STDOUT: ...
STDERR: ...
```

- [ ] **Step 3: Add repair menu after failure / 失败后增加修复菜单**

When exit code is non-zero, show:

```text
Command failed / 命令执行失败。
fix(修复) | explain(解释) | copy(复制) | quit(退出):
```

Behavior:

- `fix`: call repair model path, show revised command, then use existing execute confirmation flow.
- `explain`: summarize why it failed without executing.
- `copy`: copy failed command if clipboard is available.
- `quit`: exit.

- [ ] **Step 4: Prevent infinite loops / 防止无限循环**

Allow at most 2 repair attempts per command execution.

After 2 failures:

```text
Repair limit reached / 已达到自动修复次数上限。
Please inspect the error manually or revise the task.
请手动检查错误，或修改任务描述。
```

- [ ] **Step 5: Verify with a controlled failing command / 用可控失败命令验证**

Use a task likely to fail safely, or temporarily test with a forced failing command in development.

Expected:

```text
Failure menu appears.
fix generates a revised command.
No automatic execution happens without confirmation.
出现失败菜单。fix 生成修复命令。未经确认不会自动执行。
```

- [ ] **Step 6: Commit / 提交**

```bash
git add src/repair_cmd.rs src/main.rs assets/roles/%shell%.md
git commit -m "feat: add command repair loop"
```

---

## Task 4: Simplify user-facing command surface in docs / 简化文档中的用户命令面

**Files:**
- Modify: `/Volumes/Code/open_source/aicmd/README.md`
- Modify: `/Volumes/Code/open_source/aicmd/README.en.md`
- Modify: `/Volumes/Code/open_source/aicmd/docs/aicmd-usage.md`

- [ ] **Step 1: Keep README focused on five commands / README 聚焦五个入口**

README should make these the primary commands:

```text
aicmd <任务>
aicmd do <复杂任务>
aicmd search <查询>
aicmd setup
aicmd doctor
```

Move lower-frequency commands to a compact advanced section:

```text
config, session, update, shell-init, err
```

- [ ] **Step 2: Add examples for new features / 增加新功能示例**

Chinese README examples:

```bash
aicmd config status
aicmd --no-cache 目前内存占用率最高的程序
aicmd 当前目录有多少文件
```

Explain:

```text
如果一条命令之前成功执行过，AICmd 可能会提示复用它，以减少同一句话生成不同命令的问题。
如果命令执行失败，AICmd 会提供 fix(修复) 入口，但仍需你确认后才会执行修复命令。
```

English README counterpart:

```text
If a command succeeded before, AICmd may offer to reuse it to reduce variation for the same request.
If execution fails, AICmd offers a fix action, but the revised command still requires confirmation.
```

- [ ] **Step 3: Keep detailed reference in docs / 详细说明放 docs**

In `/Volumes/Code/open_source/aicmd/docs/aicmd-usage.md`, document:

```text
--no-cache
config status
failure repair menu
command-cache location ~/.aicmd/command-cache.yaml
```

- [ ] **Step 4: Review with karpathy-guidelines / 用 karpathy-guidelines 审查**

Checklist:

```text
- Did docs add only user-visible behavior that actually exists?
- 是否只记录了真实存在的行为？
- Is README shorter and less repetitive than before?
- README 是否更短、更少重复？
- Are English and Chinese meanings aligned?
- 中英文含义是否一致？
```

- [ ] **Step 5: Commit / 提交**

```bash
git add README.md README.en.md docs/aicmd-usage.md
git commit -m "docs: simplify AICmd usage guide"
```

---

## Task 5: Doctor improvements for release/config mismatch / 增强 doctor 的版本与配置诊断

**Files:**
- Modify: `/Volumes/Code/open_source/aicmd/src/doctor_cmd.rs`
- Modify: `/Volumes/Code/open_source/aicmd/README.md`
- Modify: `/Volumes/Code/open_source/aicmd/README.en.md`

- [ ] **Step 1: Inspect doctor output / 查看 doctor 输出**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
sed -n '1,260p' src/doctor_cmd.rs
./target/debug/aicmd doctor
```

- [ ] **Step 2: Add non-network checks only / 只增加非联网检查**

Add checks for:

```text
Config temperature: 0 / not 0 / missing
AI summary: on/off
Command cache: exists/missing
MCP config: exists/missing
Searches dir: exists/missing
```

Avoid GitHub API calls in doctor by default, so doctor remains fast and offline-friendly.

- [ ] **Step 3: Add actionable suggestions / 增加可执行建议**

Examples:

```text
Suggestion: Run `aicmd config status` to inspect active settings.
建议：运行 `aicmd config status` 查看当前配置。

Suggestion: Existing config still uses temperature 0.1. To regenerate from .env, run `aicmd init --from-env --force`.
建议：当前配置仍使用 temperature 0.1。如需从 .env 重新生成，运行 `aicmd init --from-env --force`。
```

- [ ] **Step 4: Verify / 验证**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build
./target/debug/aicmd doctor
```

- [ ] **Step 5: Commit / 提交**

```bash
git add src/doctor_cmd.rs README.md README.en.md
git commit -m "feat: improve doctor diagnostics"
```

---

## Task 6: Release v0.4.1 / 发布 v0.4.1

**Files:**
- Modify: `/Volumes/Code/open_source/aicmd/Cargo.toml`
- Modify: `/Volumes/Code/open_source/aicmd/Cargo.lock`
- Modify: `/Volumes/Code/open_source/aicmd/contrib/aicmd/install.sh`
- Modify: `/Volumes/Code/open_source/aicmd/README.md`
- Modify: `/Volumes/Code/open_source/aicmd/README.en.md`

- [ ] **Step 1: Confirm working tree / 确认工作区**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
git status --short
```

Expected allowed local uncommitted files only:

```text
 M mcp.json
?? .DS_Store
?? .aicmd/
?? assets/.DS_Store
?? tavily_key.txt
```

Do not commit these files.

- [ ] **Step 2: Bump version / 提升版本号**

Update version from `0.4.0` to `0.4.1` in:

```text
Cargo.toml
Cargo.lock
contrib/aicmd/install.sh
README.md
README.en.md
```

- [ ] **Step 3: Full verification / 完整验证**

Run:

```bash
cd /Volumes/Code/open_source/aicmd
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build
git diff --check
./target/debug/aicmd --version
./target/debug/aicmd config status
./target/debug/aicmd doctor
```

Expected:

```text
All tests pass. Version prints 0.4.1. No whitespace errors.
全部测试通过。版本显示 0.4.1。没有空白字符错误。
```

- [ ] **Step 4: Commit and tag / 提交并打标签**

```bash
git add Cargo.toml Cargo.lock contrib/aicmd/install.sh README.md README.en.md
git commit -m "chore: release v0.4.1"
git tag v0.4.1
```

- [ ] **Step 5: Push / 推送**

```bash
git push origin main
git push origin v0.4.1
```

- [ ] **Step 6: Verify GitHub release / 验证 GitHub Release**

Run:

```bash
gh run list --repo jinzheng8115/aicmd --limit 10
gh release view v0.4.1 --repo jinzheng8115/aicmd
```

Expected assets:

```text
aicmd-v0.4.1-aarch64-apple-darwin.tar.gz
aicmd-v0.4.1-x86_64-apple-darwin.tar.gz
aicmd-v0.4.1-aarch64-unknown-linux-musl.tar.gz
aicmd-v0.4.1-x86_64-unknown-linux-musl.tar.gz
```

---

## Final Verification / 最终验证

Run:

```bash
cd /Volumes/Code/open_source/aicmd
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo build
git diff --check
./target/debug/aicmd config status
./target/debug/aicmd doctor
```

Manual smoke tests:

```bash
./target/debug/aicmd --no-summary 当前目录有多少文件
./target/debug/aicmd --no-summary 当前目录有多少文件
./target/debug/aicmd --no-cache --no-summary 当前目录有多少文件
./target/debug/aicmd search docker 如何安装 --save docker_install_test
./target/debug/aicmd do --from-search docker_install_test --plan
```

Expected:

```text
- Repeated normal task offers cached successful command.
- --no-cache bypasses cache.
- config status hides secrets.
- doctor gives actionable checks.
- search/do still works.
- No sensitive local files are staged.
```

---

## Karpathy-Guidelines Review / Karpathy 指南审查

- Simplicity first / 简单优先：Plan avoids adding a broad plugin system or rewriting the CLI. Each task is focused and testable.
- Surgical changes / 外科手术式修改：The plan touches only CLI/config/cache/repair/docs files needed for the requested UX improvements.
- Explicit assumptions / 明确假设：The plan states platform scope, release mismatch, and local sensitive files.
- Verifiable success / 可验证成功：Every task includes concrete commands and expected results.
- No speculative features / 不做投机功能：Items like full agent workflows, native Windows resurrection, and complex policy engines are intentionally out of scope.

