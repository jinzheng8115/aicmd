# aicmd Terminal Workflow / aicmd 终端工作流

English: `aicmd` is a companion terminal workflow for AIChat. It keeps the upstream `aichat` binary unchanged and adds a small shell layer for daily sessions, an auto role, pre-search evidence, and agentmemory helpers.

中文：`aicmd` 是 AIChat 的终端增强工作流。它不修改上游 `aichat` 二进制，只增加一层轻量 shell 封装，用于每日会话、自动角色、预搜索证据协议和 agentmemory 辅助命令。

## What it adds / 增加了什么

English:
- Daily session by default: `aicmd hello` runs with `-s main-YYYYMMDD` using Asia/Shanghai date.
- Custom session remains explicit: `aicmd -s dev hello` uses session `dev` only for that command.
- Auto role by default: normal chat adds `-r auto`; execute mode `-e/--execute` is not changed.
- Tavily MCP pre-search for current or query-like requests.
- Evidence Protocol for time-sensitive answers: confirmed, inferred, or unconfirmed.
- Terminal-friendly output: plain text by default, no heavy Markdown formatting.
- Memory helpers: save previous output, search memories, analyze failed commands, and generate executable task scripts with confirmation.

中文：
- 默认每日会话：`aicmd hello` 会按北京时间日期使用 `-s main-YYYYMMDD`。
- 自定义会话仍然显式指定：`aicmd -s dev hello` 只在本次命令使用 `dev`。
- 默认自动角色：普通聊天自动加 `-r auto`；`-e/--execute` 命令执行模式不注入角色。
- 对时效性或查询类问题先用 Tavily MCP 预搜索。
- 时效性回答使用证据协议：已确认、可推断、未确认。
- 面向终端输出：默认纯文本，不使用重 Markdown 格式。
- 记忆辅助：保存上一条输出、搜索记忆、分析报错命令。

## Install / 安装

Prerequisites / 前置条件：
- `aichat` is already installed and configured.
- Node.js and npm are available if Tavily MCP search is used.
- `TAVILY_API_KEY` is available in the environment or in the AIChat function `.env` file.
- agentmemory is running if you want to use `aicmd-mem` or `aicmd-mem-search`.

Run / 执行：

```bash
contrib/aicmd/install.sh
```

The installer copies commands to `~/.local/bin` and copies the role/tool files to the AIChat config directory.

安装脚本会把命令复制到 `~/.local/bin`，并把 role/tool 文件复制到 AIChat 配置目录。

If your real `aichat` binary is not on `PATH`, set:

```bash
export AICMD_REAL_AICHAT=/path/to/aichat
```

如果真实的 `aichat` 不在 `PATH`，请设置上面的环境变量。

## Commands / 命令

English examples:

```bash
aicmd hello
aicmd -s dev hello
aicmd "check the latest aichat version"
aicmd -e "list files in the current directory"
aicmd -e "create scripts/process_data.py to process input.csv, then run it"
aicmd-err -- pnpm test
aicmd-do "write a script to summarize data.csv"
aicmd-mem 记录一下
aicmd-mem-search docker的命令
```

中文示例：

```bash
aicmd 你好
aicmd -s dev 继续上次 dev 会话
aicmd 查一下 aichat 最新版本
aicmd -e 列出当前目录文件
aicmd -e "创建 scripts/process_data.py 处理 input.csv，然后执行它"
aicmd-err -- pnpm test
aicmd-do "write a script to summarize data.csv"
aicmd-mem 记录一下
aicmd-mem-search docker的命令
```



## Chat vs execute mode / 对话模式和执行模式

English: Plain `aicmd` is chat mode. It answers in text and will not write scripts, modify files, or run commands by itself. Use `aicmd -e` when you want AIChat's shell assistant to produce executable shell commands. For example, ask it to create a script file and then run that script.

中文：普通 `aicmd` 是对话模式，只会用文本回答，不会自己写脚本、改文件或执行命令。如果你希望 AIChat 生成可执行 shell 命令，请使用 `aicmd -e`。比如明确要求它创建脚本文件，然后执行这个脚本。

```bash
aicmd "写个脚本处理 input.csv"
# Chat only / 只会对话说明

aicmd -e "创建 scripts/process_data.py 处理 input.csv，然后执行 python3 scripts/process_data.py"
# Execute mode / 生成可执行命令
```

## Script execution helper / 脚本执行辅助

English: Plain `aicmd` is a chat command and does not write or execute files by itself. Use `aicmd-do` when you want the model to create a script for a local file/data task. It saves the script, prints it for review, and only runs it after confirmation. Use `--yes` only when you trust the generated script.

中文：普通 `aicmd` 是聊天命令，不会自己写文件或执行脚本。如果你希望模型为本地文件/数据任务生成脚本，请使用 `aicmd-do`。它会先保存脚本并打印出来给你检查，只有确认后才执行。只有在你信任生成脚本时才使用 `--yes`。

```bash
aicmd-do "写个脚本处理 input.csv，输出 cleaned.csv"
aicmd-do --dry-run "写个脚本统计 logs/*.log 里的 ERROR 数量"
aicmd-do --yes --output scripts/process-data.sh "写个脚本处理 data.csv"
```

## Environment variables / 环境变量

English:
- `AICMD_REAL_AICHAT`: path to the upstream `aichat` executable.
- `AICHAT_CONFIG_DIR`: AIChat config directory. Defaults to `~/Library/Application Support/aichat`.
- `AICMD_STATE_DIR`: state directory for current session and last output.
- `AICMD_DEFAULT_ROLE`: default role name. Defaults to `auto`.
- `AICMD_TAVILY_TOOL`: path to `tavily_mcp_search.mjs`.
- `AICMD_AGENTMEMORY_URL`: agentmemory HTTP endpoint. Defaults to `http://localhost:3111`.
- `AICMD_MEMORY_PROJECT`: default agentmemory project. Defaults to `memory`.
- `AICHAT_NO_DEFAULT_SESSION=1`: bypass the wrapper and call real `aichat` directly.

中文：
- `AICMD_REAL_AICHAT`：上游 `aichat` 可执行文件路径。
- `AICHAT_CONFIG_DIR`：AIChat 配置目录，默认 `~/Library/Application Support/aichat`。
- `AICMD_STATE_DIR`：保存当前 session 和上一条输出的状态目录。
- `AICMD_DEFAULT_ROLE`：默认 role 名称，默认 `auto`。
- `AICMD_TAVILY_TOOL`：`tavily_mcp_search.mjs` 路径。
- `AICMD_AGENTMEMORY_URL`：agentmemory HTTP 地址，默认 `http://localhost:3111`。
- `AICMD_MEMORY_PROJECT`：默认 agentmemory 项目，默认 `memory`。
- `AICHAT_NO_DEFAULT_SESSION=1`：绕过封装，直接调用真实 `aichat`。

## Evidence Protocol / 证据协议

English: For search or time-sensitive questions, `aicmd` runs Tavily MCP before calling the model and injects the search result plus a generic evidence prompt. The answer should separate confirmed facts, inferred facts, and unconfirmed claims, and include the direct source URLs and the time basis.

中文：对查询类或时效性问题，`aicmd` 会先执行 Tavily MCP，再把搜索结果和通用证据协议注入给模型。回答应区分已确认、可推断和未确认，并给出直接来源 URL 和时间基准。

## Design boundary / 设计边界

English: This contribution intentionally starts as a companion layer instead of a Rust core rewrite. That keeps the fork easy to rebase against upstream AIChat and makes the workflow optional.

中文：这次改造有意先作为伴随层实现，而不是直接重写 Rust 核心。这样更容易跟随上游 AIChat 更新，也能让这个工作流保持可选。
