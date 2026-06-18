# AICmd P1 Config Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `aicmd config` as a lightweight config entry point and show first-time setup guidance when runtime config is missing.

**Architecture:** Add a focused `config_cmd` module for pre-config command wrapping. Adjust `Config::init` missing-config behavior so normal commands show AICmd's `.env` setup path instead of entering upstream-style interactive config creation.

**Tech Stack:** Rust, existing AICmd config/model/mcp helpers, cargo tests, bilingual README docs.

---

### Task 1: Add `aicmd config`

**Files:**
- Create: `src/config_cmd.rs`
- Modify: `src/main.rs`

- [x] **Step 1: Implement `config_cmd`**

Create `run_config_command(args: &[String]) -> Result<i32>` with these mappings:
- `help`, `-h`, `--help`, or no args: print help.
- `path`, `dir`, `show`, `edit`: delegate to `model_cmd::run_model_command`.
- `init`: delegate to `model_cmd::run_model_command` as `init --from-env`, preserving `--force`.
- `mcp`: print `AICMD_MCP_CONFIG_FILE` or `Config::config_dir().join("mcp.json")`.
- `doctor`: delegate to `doctor_cmd::run_doctor_command`.

- [x] **Step 2: Wire pre-config shortcut**

Add `mod config_cmd;` and route `aicmd config ...` in `run_pre_config_shortcut`.

- [x] **Step 3: Verify**

Run:

```bash
target/debug/aicmd config help
target/debug/aicmd config path
target/debug/aicmd config mcp
target/debug/aicmd config doctor
```

Expected: commands work without requiring a fully initialized runtime command flow.

### Task 2: First-Time Setup Guidance

**Files:**
- Modify: `src/config/mod.rs`

- [x] **Step 1: Add guidance helper**

Add a small helper that returns a clear missing-config message containing:
- Config path.
- `aicmd init --from-env`.
- `aicmd doctor`.
- Example command to try after setup.

- [x] **Step 2: Replace missing-config interactive creation**

When `Config::init` sees no config file and no dynamic provider env, return the guidance error instead of calling `create_config_file`.

- [x] **Step 3: Verify**

Run with temp config dir:

```bash
AICMD_CONFIG_DIR=$(mktemp -d) target/debug/aicmd 当前目录有多少文件
```

Expected: command exits non-zero and prints the first-time setup guidance.

### Task 3: Docs And Checks

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`

- [x] **Step 1: Document `aicmd config`**

Add command examples under model/config commands.

- [x] **Step 2: Document missing-config guidance**

Mention `aicmd config path`, `aicmd config mcp`, and `aicmd doctor` in troubleshooting.

- [x] **Step 3: Run verification**

Run:

```bash
~/.cargo/bin/cargo fmt --check
~/.cargo/bin/cargo test
git diff --check
```

Expected: all checks pass.
