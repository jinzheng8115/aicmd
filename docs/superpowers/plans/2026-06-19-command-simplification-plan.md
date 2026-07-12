# AICmd Command Simplification Implementation Plan

> **For agentic workers:** Use this plan task-by-task. Keep changes surgical. Do not delete existing commands in this phase. Documentation must remain bilingual.

**Goal:** Reduce the number of AICmd commands regular users need to remember while keeping existing capabilities compatible.

**Architecture:** Add guided follow-up actions and a setup entry point first. Defer broad natural-language intent routing until the simpler UX changes are verified.

**Tech Stack:** Rust, existing AICmd command modules, existing session/search/config helpers, bilingual README docs.

---

## Phase 1: README Task-First Rewrite / README 任务优先重写

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`

- [x] **Step 1: Reorder README / 调整 README 信息架构**

Move the full command index behind the task/workflow sections.

Suggested order:

```text
1. Installation / 安装
2. First-time setup / 首次配置
3. Most common entry points / 最常用入口
4. Common tasks / 常见任务
5. Common workflows / 常见工作流
6. Advanced command reference / 高级命令参考
7. Safety notes / 安全注意事项
8. Troubleshooting / 排障
```

- [x] **Step 2: Introduce “remember only these” / 强调只需记住少量入口**

Highlight:

```text
aicmd <task>
aicmd do <task>
aicmd search <query>
aicmd setup
aicmd doctor
```

- [x] **Step 3: Demote advanced commands / 降级高级命令**

Keep `session`, `last`, `config`, `model`, `mcp`, `mcp-raw`, `shell-init`, `update`, `--dry-run`, and `--print` documented, but label them as advanced/troubleshooting commands.

- [x] **Verification / 验证**

Run:

```bash
git diff --check
```

Manual review:
- A new user sees the core workflow before the full command table.
- README.md and README.en.md contain equivalent content.

---

## Phase 2: Search Follow-Up Menu / 搜索后交互菜单

**Files:**
- Modify: `src/main.rs`
- Modify: `src/search_cmd.rs` if helper reuse is needed
- Modify: `README.md`
- Modify: `README.en.md`

- [x] **Step 1: Add post-search menu / 增加搜索后菜单**

After `aicmd search <query>` completes summarization and persists `.last.txt` / `.last.raw.txt`, show a terminal menu when stdout is interactive:

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

Non-interactive behavior:
- Do not show the menu.
- Keep current behavior unchanged.

- [x] **Step 2: Implement save action / 实现保存**

Behavior:
- Ask for a name with a short prompt.
- Empty name means auto-generate.
- Reuse existing search persistence helpers.

- [x] **Step 3: Implement do action / 实现基于搜索执行**

Behavior:
- Ask for the follow-up task text.
- Use the saved `.last.txt` search result as `do` context.
- Enter the normal `aicmd do` generated-command confirmation flow.

Safety:
- Do not auto-execute generated commands.
- Continue using risk hints and confirmation.

- [x] **Step 4: Implement open action / 实现打开**

Behavior:
- Open `.last.txt` using existing open behavior if available.
- If no opener is available, print the path.

- [x] **Step 5: Tests / 测试**

Add or update tests for argument parsing and helper behavior where practical.

Run:

```bash
cargo fmt --check
cargo test
git diff --check
```

Manual checks:

```bash
aicmd search "Docker 如何安装"
# choose save
# choose do
# choose open
```

---

## Phase 3: `aicmd setup` / 配置入口收口

**Files:**
- Create or modify: `src/setup_cmd.rs`
- Modify: `src/main.rs`
- Modify: `README.md`
- Modify: `README.en.md`

- [x] **Step 1: Add pre-config shortcut / 增加 pre-config 入口**

Route `aicmd setup` before requiring a valid runtime config.

- [x] **Step 2: Implement lightweight setup / 实现轻量向导**

Behavior:
- Print current config path.
- If `config.yaml` exists, explain how to inspect or regenerate it.
- If `.env` or `AICMD_MODEL_ENV` exists, offer to run the same path as `aicmd init --from-env`.
- If local `mcp.json` exists, offer to copy it to `~/.aicmd/mcp.json`.
- End with `aicmd doctor` guidance, or run doctor when safe.

Safety:
- Overwriting `config.yaml` or `mcp.json` requires confirmation.
- Never print API keys.

- [x] **Step 3: Keep compatibility / 保持兼容**

Do not remove:
- `aicmd init --from-env`
- `aicmd config ...`
- `aicmd model ...`

Docs should recommend `aicmd setup` for regular users and keep the older commands in advanced reference.

- [x] **Step 4: Tests / 测试**

Run with temporary config directories:

```bash
AICMD_CONFIG_DIR=$(mktemp -d) target/debug/aicmd setup
AICMD_CONFIG_DIR=$(mktemp -d) AICMD_MODEL_ENV=/path/to/.env target/debug/aicmd setup
```

Run:

```bash
cargo fmt --check
cargo test
git diff --check
```

---

## Phase 4: Small Natural-Language System Intents / 小范围自然语言系统意图

Status / 状态：partially implemented. The first narrow batch supports saving the latest search, using it as `do` context, and showing recent session messages. Session switching and clearing remain deferred because their state and destructive semantics need a separate narrow design.

状态：部分完成。第一批支持保存最近搜索、将最近搜索作为 `do` 上下文，以及查看最近 session 消息。会话切换和清空仍暂缓，因为其状态和破坏性语义需要单独设计。

**Files:**
- Modify: `src/main.rs`
- Possibly create: `src/intent_cmd.rs`
- Modify docs after behavior is implemented.

- [ ] **Step 1: Define narrow intent patterns / 定义窄范围意图**

Start with explicit, low-ambiguity Chinese and English patterns:

```text
保存刚才的搜索结果
用刚才的搜索结果 <task>
查看最近 N 条上下文
清空当前会话
切换到 <name> 会话
```

- [ ] **Step 2: Route before shell command generation / 在生成 shell 命令前路由**

If a pattern matches with high confidence, run the matching system operation.
If uncertain, do not execute; print a suggested explicit command.

- [ ] **Step 3: Safety confirmations / 安全确认**

Require confirmation for:
- Clearing sessions.
- Deleting saved search records.
- Overwriting config or MCP files.

- [ ] **Step 4: Tests / 测试**

Add unit tests for the pattern matcher.

Run:

```bash
cargo fmt --check
cargo test
git diff --check
```

---

## Phase 5: Review And Release / 复盘与发布

- [ ] **Step 1: Review with karpathy-guidelines / 使用 karpathy-guidelines review**

Check:
- Are changes minimal?
- Did we avoid deleting compatible commands?
- Are user-facing docs simpler than before?
- Are safety confirmations preserved?

- [ ] **Step 2: Update memory / 更新 agentmemory**

Save:
- Final implemented UX behavior.
- Any root cause discovered during implementation.
- Files modified and verification results.

- [ ] **Step 3: Push / 推送**

Run:

```bash
git status -sb
git add <intended files>
git commit -m "Simplify AICmd command UX"
git push origin main
```

Only stage intended files. Do not stage local secrets such as `.env` or real `mcp.json` keys.
