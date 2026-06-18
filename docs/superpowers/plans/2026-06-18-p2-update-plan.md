# AICmd P2 Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `aicmd update` so users can update AICmd without remembering installer URLs.

**Architecture:** Add a focused `update_cmd` module that builds the platform installer command, supports dry-run and version forwarding, asks for confirmation, and runs the existing installer. Wire it as a pre-config command so updates work even when config is missing.

**Tech Stack:** Rust std process APIs, existing installer scripts, cargo checks/tests, bilingual README docs.

---

### Task 1: Implement `update_cmd`

**Files:**
- Create: `src/update_cmd.rs`

- [x] **Step 1: Parse args**

Support:
- no args
- `--version <version>`
- `--version=<version>`
- `--dry-run`
- `help`, `-h`, `--help`

- [x] **Step 2: Build commands**

For macOS/Linux:

```bash
sh -c 'curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash'
```

With version:

```bash
sh -c 'curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.3'
```

For Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex"
```

With version, download to temp file and invoke `-Version`.

- [x] **Step 3: Confirm and execute**

Ask for confirmation unless `--dry-run` is set. Execute installer with inherited stdio. After success, print `Run: aicmd doctor`.

### Task 2: Wire Command

**Files:**
- Modify: `src/main.rs`

- [x] **Step 1: Add module**

Add `mod update_cmd;`.

- [x] **Step 2: Add pre-config route**

Route `aicmd update ...` before config loading.

- [x] **Step 3: Verify dry-run**

Run:

```bash
target/debug/aicmd update --dry-run
target/debug/aicmd update --version v0.30.3 --dry-run
```

Expected: prints installer command and exits without network/file writes.

### Task 3: Docs And Checks

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`

- [x] **Step 1: Document update**

Add `aicmd update` and `aicmd update --version ...` under update docs.

- [x] **Step 2: Run checks**

Run:

```bash
~/.cargo/bin/cargo fmt --check
~/.cargo/bin/cargo test
git diff --check
```

Expected: all pass.
