# AICmd P1 Config Onboarding Design

中文：本文定义 P1 UX 改进：新增 `aicmd config` 轻量入口，并在缺少 `config.yaml` 时显示首次使用引导。目标是降低安装后的配置理解成本。

English: This document defines the P1 UX improvement: add a lightweight `aicmd config` entry point and show first-time setup guidance when `config.yaml` is missing. The goal is to reduce setup confusion after installation.

## Goals / 目标

中文：
- 用户可以通过 `aicmd config ...` 发现和管理常见配置操作。
- 缺少 `~/.aicmd/config.yaml` 时，AICmd 给出清晰的 `.env -> aicmd init --from-env -> aicmd doctor` 路径。
- 保持现有 `aicmd model ...` 和 `aicmd init ...` 行为兼容。

English:
- Users can discover and run common config operations through `aicmd config ...`.
- When `~/.aicmd/config.yaml` is missing, AICmd shows the `.env -> aicmd init --from-env -> aicmd doctor` path.
- Existing `aicmd model ...` and `aicmd init ...` behavior remains compatible.

## Non-Goals / 非目标

中文：
- 本期不做复杂 TUI。
- 本期不新增真实联网测试，例如 `config test-search`。
- 本期不改变 `config.yaml`、`.env` 或 `mcp.json` 的格式。
- 本期不实现 `aicmd update`。

English:
- This phase does not add a complex TUI.
- This phase does not add live network tests such as `config test-search`.
- This phase does not change `config.yaml`, `.env`, or `mcp.json` formats.
- This phase does not implement `aicmd update`.

## Feature 1: `aicmd config`

中文：新增 pre-config 子命令 `aicmd config`。它是现有命令的薄包装，优先保证可发现性和易记性。

English: Add a pre-config `aicmd config` subcommand. It is a thin wrapper over existing commands, prioritizing discoverability and memorability.

Commands / 命令：

```text
aicmd config path          # same as aicmd model path
aicmd config dir           # same as aicmd model dir
aicmd config show          # same as aicmd model show
aicmd config edit          # same as aicmd model edit
aicmd config init          # same as aicmd init --from-env
aicmd config init --force  # same as aicmd init --from-env --force
aicmd config mcp           # print ~/.aicmd/mcp.json or AICMD_MCP_CONFIG_FILE
aicmd config doctor        # same as aicmd doctor
aicmd config help          # print help
```

Behavior / 行为：
- `config init` always uses `--from-env`.
- `config mcp` only prints the MCP path; it does not create or edit files.
- Unknown options should print compact help and return non-zero.

## Feature 2: First-Time Setup Guidance

中文：如果用户运行普通自然语言命令时缺少 `config.yaml`，AICmd 应直接显示首次使用引导，而不是进入不符合当前产品路径的上游式配置交互。

English: If the user runs a normal natural-language command without `config.yaml`, AICmd should show first-time setup guidance instead of entering an upstream-style config creation flow that does not match the current product path.

Message / 提示：

```text
AICmd config not found: ~/.aicmd/config.yaml

First-time setup:
1. Create a .env file with your model settings.
2. Run: aicmd init --from-env
3. Check: aicmd doctor
4. Try: aicmd 当前目录有多少文件
```

Allowed without config / 无配置时仍允许：
- `aicmd init ...`
- `aicmd model ...`
- `aicmd config ...`
- `aicmd doctor`
- `aicmd shell-init`
- `aicmd mcp list/help`
- `aicmd mcp-raw ...`

## Testing / 测试

Manual checks / 手动检查：
- `aicmd config help`
- `aicmd config path`
- `aicmd config mcp`
- `aicmd config doctor`
- `aicmd config init --force` with a temp config directory and `.env`
- Missing-config normal command with temp `AICMD_CONFIG_DIR`

Automated checks / 自动检查：
- `cargo fmt --check`
- `cargo test`
- `git diff --check`

## Product Rationale / 产品理由

中文：`aicmd doctor` 已经能诊断问题，但用户还需要一个自然的配置入口。`aicmd config` 提供“从哪里看、在哪里改、怎么初始化”的低认知负担入口；首次引导则减少安装后第一次运行的卡点。

English: `aicmd doctor` can diagnose issues, but users still need a natural config entry point. `aicmd config` provides a low-friction path for where to inspect, edit, and initialize config; first-time guidance removes the first-run dead end after installation.
