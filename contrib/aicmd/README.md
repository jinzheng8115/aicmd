# AICmd Command Workflow / AICmd 命令工作流

English: `aicmd` is a command-first terminal workflow. Plain `aicmd` defaults to natural-language shell execution. Chat mode is explicit with `aicmd chat ...` or `aicmd-chat ...`.

中文：`aicmd` 是命令优先的终端工作流。普通 `aicmd` 默认进入自然语言 shell 执行模式。聊天模式需要显式使用 `aicmd chat ...` 或 `aicmd-chat ...`。

## What it adds / 增加了什么

English:
- Default command mode: plain `aicmd ...` injects `-e` and asks before running the generated shell command.
- Daily command session: plain `aicmd ...` runs with `-s cmd-YYYYMMDD` using Asia/Shanghai date.
- Custom session remains explicit: `aicmd -s dev ...` uses session `dev` only for that command.
- Chat fallback: `aicmd chat ...` uses the auto role and terminal-friendly output.
- Script helper: `aicmd-do` generates a script, saves it, prints it, and runs only after confirmation.
- Error helper: `aicmd-err -- <command>` captures stdout/stderr/exit code and asks AICmd to analyze it.
- Optional memory helpers: `aicmd-mem` and `aicmd-mem-search` integrate with agentmemory.

中文：
- 默认命令模式：普通 `aicmd ...` 自动注入 `-e`，并在执行生成的 shell 命令前让你确认。
- 默认每日命令会话：普通 `aicmd ...` 会按北京时间日期使用 `-s cmd-YYYYMMDD`。
- 自定义会话仍然显式指定：`aicmd -s dev ...` 只在本次命令使用 `dev`。
- 聊天回退：`aicmd chat ...` 使用 auto role 和终端友好输出。
- 脚本辅助：`aicmd-do` 生成脚本、保存脚本、打印检查，并只在确认后执行。
- 报错辅助：`aicmd-err -- <command>` 捕获 stdout/stderr/exit code，并让 AICmd 分析。
- 可选记忆辅助：`aicmd-mem` 和 `aicmd-mem-search` 对接 agentmemory。

## Install / 安装

Prerequisites / 前置条件：
- `aichat` compatible binary is already installed and configured.
- Node.js and npm are available if Tavily MCP search is used in chat mode.
- `TAVILY_API_KEY` is available in the environment or in the AIChat function `.env` file if search is used.
- agentmemory is running if you want to use `aicmd-mem` or `aicmd-mem-search`.

Run / 执行：

```bash
contrib/aicmd/install.sh
```

The installer copies commands to `~/.local/bin` and copies the role/tool files to the AIChat config directory.

安装脚本会把命令复制到 `~/.local/bin`，并把 role/tool 文件复制到 AIChat 配置目录。

If your real binary is not on `PATH`, set:

```bash
export AICMD_REAL_AICHAT=/path/to/aichat
```

如果真实二进制不在 `PATH`，请设置上面的环境变量。

## Commands / 命令

English examples:

```bash
aicmd list the largest 10 files in the current directory
aicmd compress png files in the current directory into dist/images
aicmd -s dev run tests and fix obvious failures
aicmd chat explain the difference between tar and gzip
aicmd-do "write a script to summarize data.csv"
aicmd-err -- pnpm test
aicmd-mem 记录一下
aicmd-mem-search docker的命令
```

中文示例：

```bash
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题
aicmd chat 解释 tar 和 gzip 的区别
aicmd-do "写个脚本处理 input.csv，输出 cleaned.csv"
aicmd-err -- pnpm test
aicmd-mem 记录一下
aicmd-mem-search docker的命令
```

## Chat vs command mode / 聊天模式和命令模式

English: Plain `aicmd` is command mode. It generates a shell command and asks before execution. Use `aicmd chat ...` only when you want explanation or general conversation.

中文：普通 `aicmd` 是命令模式，会生成 shell 命令并在执行前确认。只有需要解释或普通对话时，才使用 `aicmd chat ...`。

```bash
aicmd "写个脚本处理 input.csv"
# Command mode / 命令模式：生成 shell 命令并确认执行

aicmd chat "写脚本处理 input.csv 有哪些思路"
# Chat mode / 聊天模式：只解释，不执行
```

## Script execution helper / 脚本执行辅助

English: Use `aicmd-do` when you want the model to create a script for a local file/data task. It saves the script, prints it for review, and only runs it after confirmation. Use `--yes` only when you trust the generated script.

中文：如果你希望模型为本地文件/数据任务生成脚本，请使用 `aicmd-do`。它会先保存脚本并打印出来给你检查，只有确认后才执行。只有在你信任生成脚本时才使用 `--yes`。

```bash
aicmd-do "写个脚本处理 input.csv，输出 cleaned.csv"
aicmd-do --dry-run "写个脚本统计 logs/*.log 里的 ERROR 数量"
aicmd-do --yes --output scripts/process-data.sh "写个脚本处理 data.csv"
```

## Environment variables / 环境变量

English:
- `AICMD_REAL_AICHAT`: path to the compatible executable.
- `AICHAT_CONFIG_DIR`: config directory. Defaults to `~/Library/Application Support/aichat`.
- `AICMD_STATE_DIR`: state directory for current session and last output.
- `AICMD_SESSION_PREFIX`: default session prefix. Defaults to `cmd`.
- `AICMD_DEFAULT_ROLE`: chat fallback role name. Defaults to `auto`.
- `AICMD_TAVILY_TOOL`: path to `tavily_mcp_search.mjs`.
- `AICMD_AGENTMEMORY_URL`: agentmemory HTTP endpoint. Defaults to `http://localhost:3111`.
- `AICMD_MEMORY_PROJECT`: default agentmemory project. Defaults to `memory`.
- `AICHAT_NO_DEFAULT_SESSION=1`: bypass the wrapper and call the real binary directly.

中文：
- `AICMD_REAL_AICHAT`：兼容二进制路径。
- `AICHAT_CONFIG_DIR`：配置目录，默认 `~/Library/Application Support/aichat`。
- `AICMD_STATE_DIR`：保存当前 session 和上一条输出的状态目录。
- `AICMD_SESSION_PREFIX`：默认 session 前缀，默认 `cmd`。
- `AICMD_DEFAULT_ROLE`：聊天回退 role 名称，默认 `auto`。
- `AICMD_TAVILY_TOOL`：`tavily_mcp_search.mjs` 路径。
- `AICMD_AGENTMEMORY_URL`：agentmemory HTTP 地址，默认 `http://localhost:3111`。
- `AICMD_MEMORY_PROJECT`：默认 agentmemory 项目，默认 `memory`。
- `AICHAT_NO_DEFAULT_SESSION=1`：绕过封装，直接调用真实二进制。

## Design boundary / 设计边界

English: This is currently a focused companion layer over the upstream-compatible binary. The project surface is command-first, while deeper Rust-level removal of unused upstream modules can happen incrementally after behavior is stable.

中文：当前实现是基于兼容上游二进制的聚焦伴随层。项目入口已经命令优先；更深层的 Rust 模块删除可以在行为稳定后逐步进行。
