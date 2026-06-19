# AICmd Command Simplification Design

中文：本文定义 AICmd 下一阶段的产品方向：减少用户需要记住的 AICmd 命令，让用户主要通过自然语言和少量核心入口完成终端任务。

English: This document defines the next AICmd product direction: reduce the number of AICmd commands users must remember, and let users complete terminal tasks through natural language plus a few core entry points.

## Problem / 问题

中文：AICmd 的目标是让用户少记终端命令。但随着 `search`、`do`、`session`、`config`、`model`、`mcp`、`update` 等能力增加，用户又需要记住很多 AICmd 自己的命令。这违背了产品初衷。

English: AICmd aims to help users remember fewer terminal commands. But as capabilities such as `search`, `do`, `session`, `config`, `model`, `mcp`, and `update` were added, users now need to remember many AICmd-specific commands. This conflicts with the product goal.

## Product Principle / 产品原则

中文：保留能力，收口入口。高级命令可以继续存在，但普通用户不应该需要记住它们。

English: Keep the capabilities, reduce the entry points. Advanced commands may continue to exist, but regular users should not need to remember them.

## Goals / 目标

中文：
- 普通用户只需要记住 3-5 个主要入口。
- 常见后续动作通过交互菜单承接，而不是要求用户记新命令。
- 配置、模型、MCP、session 等系统操作尽量通过 `setup`、`doctor` 或自然语言引导完成。
- README 从“命令手册”改成“任务手册”，完整命令索引降级为高级参考。
- 现有命令保持兼容，不在本阶段删除。

English:
- Regular users only need to remember 3-5 primary entry points.
- Common follow-up actions are handled by interactive menus instead of requiring users to remember more commands.
- System operations such as config, model, MCP, and session management should be guided through `setup`, `doctor`, or natural-language intent where possible.
- README should shift from a command manual to a task manual, with the full command index moved to advanced reference.
- Existing commands remain compatible; this phase does not remove them.

## Non-Goals / 非目标

中文：
- 本阶段不删除 `config`、`model`、`mcp`、`session` 等已有命令。
- 本阶段不引入复杂 TUI。
- 本阶段不恢复 Windows 原生支持；Windows 用户仍建议使用 WSL。
- 本阶段不改变 `config.yaml`、`.env` 或 `mcp.json` 的格式。
- 本阶段不实现完全通用的自然语言路由器，只先覆盖高频、低风险场景。

English:
- This phase does not delete existing commands such as `config`, `model`, `mcp`, or `session`.
- This phase does not introduce a complex TUI.
- This phase does not restore native Windows support; Windows users should continue to use WSL.
- This phase does not change the formats of `config.yaml`, `.env`, or `mcp.json`.
- This phase does not implement a fully general natural-language router; it first covers high-frequency, low-risk cases.

## Primary User Surface / 普通用户入口

中文：普通用户文档和帮助中优先呈现以下入口：

```text
aicmd <自然语言任务>
aicmd do <复杂任务>
aicmd search <需要联网或 MCP 的问题>
aicmd setup
aicmd doctor
```

English: User-facing documentation and help should prioritize these entry points:

```text
aicmd <natural-language task>
aicmd do <complex task>
aicmd search <web/MCP-backed question>
aicmd setup
aicmd doctor
```

## Advanced Surface / 高级入口

中文：以下命令保留，但在 README 中降级为“高级参考 / 排障命令”：

```text
aicmd session
aicmd last
aicmd config
aicmd model
aicmd mcp
aicmd mcp-raw
aicmd shell-init
aicmd update
aicmd --dry-run
aicmd --print
```

English: The following commands remain available, but README should present them as advanced or troubleshooting references:

```text
aicmd session
aicmd last
aicmd config
aicmd model
aicmd mcp
aicmd mcp-raw
aicmd shell-init
aicmd update
aicmd --dry-run
aicmd --print
```

## Feature 1: Search Follow-Up Menu / 搜索后的后续菜单

中文：`aicmd search <query>` 输出整理结果后，提供交互菜单：

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

行为：
- `save`：保存当前搜索结果；如果用户输入名称则使用该名称，否则自动生成名称。
- `do`：把当前搜索结果作为上下文进入 `aicmd do` 流程。
- `open`：打开当前保存的 `.last.txt` 搜索结果。
- `quit`：退出。

English: After `aicmd search <query>` prints the summarized result, show an interactive follow-up menu:

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

Behavior:
- `save`: save the current search result; use a user-provided name when present, otherwise auto-generate one.
- `do`: enter the `aicmd do` workflow with the current search result as context.
- `open`: open the current `.last.txt` search result.
- `quit`: exit.

## Feature 2: `aicmd setup` / 配置入口收口

中文：新增 `aicmd setup` 作为普通用户的配置入口。它不是复杂 TUI，而是轻量向导，负责把用户带到可运行状态。

English: Add `aicmd setup` as the regular-user setup entry point. It is not a complex TUI; it is a lightweight wizard that gets the user to a runnable state.

Scope / 范围：
- 检查 `~/.aicmd/config.yaml` 是否存在。
- 支持从当前目录 `.env` 或 `AICMD_MODEL_ENV` 生成配置。
- 如果存在 `mcp.json`，提示是否复制到 `~/.aicmd/mcp.json`。
- 最后自动运行或提示运行 `aicmd doctor`。
- 对覆盖已有配置的行为保持二次确认。

## Feature 3: Natural-Language System Intents / 自然语言系统意图

中文：逐步支持少量系统操作的自然语言表达，例如：

```text
aicmd 保存刚才的搜索结果
aicmd 用刚才的搜索结果安装 Docker
aicmd 查看最近 5 条上下文
aicmd 清空当前会话
aicmd 切换到 dev 会话
```

English: Gradually support a small set of natural-language system operations, for example:

```text
aicmd save the previous search result
aicmd use the previous search result to install Docker
aicmd show the last 5 context messages
aicmd clear the current session
aicmd switch to session dev
```

Safety / 安全：
- 清空 session、覆盖配置、删除搜索记录等操作仍必须二次确认。
- 识别不确定时，不执行系统操作；给出建议命令或要求用户确认。

## README Information Architecture / README 信息架构

中文：README 应从“完整命令列表优先”改为“任务优先”：

```text
1. 安装
2. 首次配置
3. 五个最常用入口
4. 常见任务：文件、系统状态、搜索、报错修复、复杂脚本
5. 常见工作流：搜索 -> 保存/执行，失败 -> 修复
6. 高级命令参考
7. 安全注意事项
8. 排障
```

English: README should move from “full command list first” to “task first”:

```text
1. Installation
2. First-time setup
3. Five common entry points
4. Common tasks: files, system status, search, error fixing, complex scripts
5. Common workflows: search -> save/run, failure -> revise
6. Advanced command reference
7. Safety notes
8. Troubleshooting
```

## Success Criteria / 成功标准

中文：
- 新用户读 README 前半部分即可完成安装、配置、第一次运行、搜索和复杂任务。
- 常见“搜索后保存/执行”不再要求用户记住 `search save` 或 `do --from-search`。
- `setup` 成为配置相关文档的首选入口。
- 完整命令仍可查，但不会成为新用户必须学习的第一屏内容。

English:
- New users can complete install, setup, first run, search, and complex tasks by reading the first half of README.
- Common “search then save/run” flows no longer require remembering `search save` or `do --from-search`.
- `setup` becomes the preferred entry point for configuration docs.
- The full command reference remains available, but is no longer the first thing new users must learn.
