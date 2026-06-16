# AICmd: Natural Language Terminal Commands

AICmd turns natural language into terminal commands. It is derived from [sigoden/aichat](https://github.com/sigoden/aichat), but this project focuses on one workflow: describe what you want in plain language, review the generated shell command, then run it.

AICmd 用自然语言运行终端命令。它基于 [sigoden/aichat](https://github.com/sigoden/aichat) 改造，但本项目聚焦一个工作流：用自然语言描述你要做的事，检查生成的 shell 命令，然后执行。

Original upstream / 原始上游：sigoden/aichat
License / 许可证：MIT OR Apache-2.0, following upstream licensing.

## Focus / 项目聚焦

English:
- Default command mode: `aicmd 列出当前目录最大的 10 个文件` generates a shell command and asks before running it.
- Safe review loop: generated commands are shown before execution by upstream AIChat shell assistant behavior.
- Daily command session: plain `aicmd` uses `cmd-YYYYMMDD` by default.
- Script workflow: `aicmd-do` generates a script, saves it, prints it for review, and runs only after confirmation.
- Optional chat fallback: use `aicmd chat ...` or `aicmd-chat ...` only when you need explanation instead of command execution.

中文：
- 默认命令模式：`aicmd 列出当前目录最大的 10 个文件` 会生成 shell 命令，并在执行前让你确认。
- 安全确认流程：沿用上游 AIChat shell assistant 的执行前确认机制。
- 每日命令会话：普通 `aicmd` 默认使用 `cmd-YYYYMMDD`。
- 脚本工作流：`aicmd-do` 会生成脚本、保存脚本、打印检查，并只在确认后执行。
- 可选聊天回退：只有需要解释而不是执行命令时，使用 `aicmd chat ...` 或 `aicmd-chat ...`。

## Install / 安装

Build or install the upstream-compatible binary first, then install the AICmd helper commands:

先构建或安装兼容上游的二进制，然后安装 AICmd 辅助命令：

```bash
# If you use the upstream Homebrew binary:
brew install aichat

# Install AICmd shell helpers from this repo:
contrib/aicmd/install.sh
```

If the real binary is not named `aichat` or is not on PATH, set:

如果真实二进制不叫 `aichat` 或不在 PATH 中，请设置：

```bash
export AICMD_REAL_AICHAT=/path/to/aichat
```

## Usage / 使用

```bash
# Generate and confirm a shell command / 生成并确认 shell 命令
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题

# Explicit execute mode still works / 显式 -e 仍然可用
aicmd -e 列出当前目录文件

# Chat only / 仅聊天解释
aicmd chat 解释一下 tar 和 gzip 的区别
aicmd-chat 解释一下 chmod 755 是什么意思

# Generate a script, review it, then run after confirmation / 生成脚本、检查、确认后执行
aicmd-do "写个脚本处理 input.csv，输出 cleaned.csv"
aicmd-do --dry-run "写个脚本统计 logs/*.log 里的 ERROR 数量"

# Debug a failing command / 分析报错命令
aicmd-err -- pnpm test
```

## What was intentionally de-emphasized / 有意弱化的上游功能

AICmd still keeps much of the upstream codebase for compatibility, but the product surface is intentionally narrowed. General chat, REPL, RAG, agents, macros, built-in server, and broad LLM playground features are no longer the main workflow. They may still exist internally while the project is being reduced, but new documentation and helper commands focus on natural-language terminal execution.

AICmd 目前仍保留大量上游代码以维持兼容，但产品入口已经收窄。通用聊天、REPL、RAG、agents、macros、内置 server 和 LLM playground 不再是主工作流。在项目继续瘦身期间，它们可能仍然存在于内部代码里，但新的文档和辅助命令只聚焦自然语言终端执行。

## Command helpers / 命令辅助

- `aicmd`: default natural-language command execution.
- `aicmd-chat`: explicit chat/explanation mode.
- `aicmd-do`: generate a task script, print it, then run after confirmation.
- `aicmd-err`: run a command, capture stdout/stderr/exit code, and ask AICmd to analyze it.
- `aicmd-mem` and `aicmd-mem-search`: optional agentmemory helpers.

## Upstream reference / 上游参考

See `docs/upstream-aichat.md` for the preserved upstream AIChat feature overview.

上游 AIChat 原功能概览保存在 `docs/upstream-aichat.md`。
