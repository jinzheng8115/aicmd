# AICmd: Natural Language Terminal Commands

AICmd turns natural language into terminal commands. It is derived from [sigoden/aichat](https://github.com/sigoden/aichat), but this project focuses on one workflow: describe what you want in plain language, review the generated shell command, then run it.

AICmd 用自然语言运行终端命令。它基于 [sigoden/aichat](https://github.com/sigoden/aichat) 改造，但本项目聚焦一个工作流：用自然语言描述你要做的事，检查生成的 shell 命令，然后执行。

Original upstream / 原始上游：sigoden/aichat
License / 许可证：MIT OR Apache-2.0, following upstream licensing.

## Focus / 项目聚焦

English:
- The Rust binary is now `aicmd`.
- Default workflow: `aicmd 列出当前目录最大的 10 个文件` generates a shell command and asks before running it.
- Daily command session: plain `aicmd` uses `cmd-YYYYMMDD` by default.
- Script workflow: `aicmd-do` asks AICmd to generate commands that create and run a task script through the normal confirmation flow.
- Error workflow: `aicmd-err -- <command>` captures command output and asks AICmd to analyze it.
- Broad upstream features such as REPL, RAG, agents, macros, custom roles, and server mode are not part of the public AICmd CLI surface.

中文：
- Rust 二进制现在叫 `aicmd`。
- 默认工作流：`aicmd 列出当前目录最大的 10 个文件` 会生成 shell 命令，并在执行前让你确认。
- 每日命令会话：普通 `aicmd` 默认使用 `cmd-YYYYMMDD`。
- 脚本工作流：`aicmd-do` 会让 AICmd 生成“创建并运行任务脚本”的命令，并走正常确认流程。
- 报错工作流：`aicmd-err -- <command>` 捕获命令输出，并让 AICmd 分析。
- REPL、RAG、agents、macros、自定义 roles、server mode 等上游宽功能不属于 AICmd 的公开 CLI 使用面。

## Install / 安装

```bash
contrib/aicmd/install.sh
```

The installer builds the Rust binary and copies `aicmd`, `aicmd-do`, and `aicmd-err` to `~/.local/bin` by default.

安装脚本会构建 Rust 二进制，并默认把 `aicmd`、`aicmd-do`、`aicmd-err` 复制到 `~/.local/bin`。

## Usage / 使用

```bash
# Generate and confirm a shell command / 生成并确认 shell 命令
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题

# Generate a script, review it, then run after confirmation / 生成脚本、检查、确认后执行
aicmd-do "处理 input.csv，输出 cleaned.csv"
aicmd-do --dry-run "统计 logs/*.log 里的 ERROR 数量"

# Debug a failing command / 分析报错命令
aicmd-err -- pnpm test
```

## Compatibility / 兼容性

AICmd still reuses upstream AIChat internals for model providers, config loading, sessions, roles, and shell execution. To avoid breaking existing setups, config lookup prefers `AICMD_CONFIG_DIR`, then `AICHAT_CONFIG_DIR`, then an existing `aichat` config directory if present.

AICmd 仍复用上游 AIChat 的 provider、配置加载、session、role 和 shell 执行内部能力。为了避免破坏现有配置，配置目录查找顺序为：`AICMD_CONFIG_DIR`、`AICHAT_CONFIG_DIR`、已有的 `aichat` 配置目录。

## Upstream reference / 上游参考

See `docs/upstream-aichat.md` for the preserved upstream AIChat feature overview.

上游 AIChat 原功能概览保存在 `docs/upstream-aichat.md`。
