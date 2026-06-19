# AICmd Usage Guide / AICmd 使用文档

This document is the detailed command reference for AICmd. The README is the short start guide; this file keeps the longer explanations.

本文档是 AICmd 的详细命令参考。README 是快速开始指南；更完整的说明放在这里。

## 1. Product scope / 产品范围

AICmd is a natural-language terminal command runner. You describe a terminal task, AICmd generates a reviewable command or script, and you decide whether to execute it.

AICmd 是自然语言终端命令工具。你描述终端任务，AICmd 生成可检查的命令或脚本，然后由你决定是否执行。

AICmd is intentionally focused. It does not expose broad upstream AIChat workflows such as Chat REPL, RAG, agents, macros, public role switching, custom tools/functions, or server mode.

AICmd 刻意保持聚焦。它不暴露上游 AIChat 的宽功能，例如 Chat REPL、RAG、agents、macros、公开 role 切换、自定义 tools/functions 或 server mode。

## 2. Supported platforms / 支持平台

AICmd officially supports macOS, Linux, and Windows WSL. Native Windows PowerShell/cmd is not supported.

AICmd 正式支持 macOS、Linux 和 Windows WSL。不支持 Windows 原生 PowerShell/cmd。

## 3. Install / 安装

Recommended binary install, no Rust required:

推荐二进制安装，不需要 Rust：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

If the repository is already cloned locally, run the same installer from the checkout. It still downloads the release binary by default and does not require Rust:

如果已经 clone 了仓库，可以直接运行本地安装脚本。默认仍下载 release 二进制，不需要 Rust：

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
contrib/aicmd/install.sh
```

Default paths:

默认路径：

```text
~/.local/bin/aicmd              binary / 主程序
~/.aicmd/config.yaml            model/runtime config / 模型和运行配置
~/.aicmd/mcp.json               MCP config / MCP 配置
~/.aicmd/sessions/              session history / 会话记录
~/.aicmd/searches/              saved search results / 搜索记录
~/.aicmd/command-cache.yaml     successful command cache / 成功命令缓存
```

Refresh shell command cache after installation if needed:

如 shell 缓存了旧命令，安装后刷新：

```bash
hash -r
```

## 4. First-time configuration / 首次配置

Prepare `.env`, then generate runtime config:

准备 `.env`，然后生成运行配置：

```bash
aicmd setup
# or / 或者
aicmd init --from-env
```

Overwrite existing config with confirmation:

覆盖已有配置，会二次确认：

```bash
aicmd init --from-env --force
```

Minimal OpenAI-compatible `.env`:

最小 OpenAI 兼容 `.env`：

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
AICMD_DEFAULT_MODEL=deepseek:deepseek-chat
```

Generated defaults include:

生成的默认值包括：

```yaml
temperature: 0
top_p: null
stream: false
ai_summary: true
```

## 5. Main commands / 主要命令

Start with these five commands:

优先记住这五个入口：

```bash
aicmd <task>          # generate one command / 生成一条命令
aicmd do <task>       # complex script task / 复杂脚本任务
aicmd search <query>  # MCP search + LLM summary / MCP 搜索 + LLM 整理
aicmd setup           # first-time setup / 首次配置
aicmd doctor          # diagnose install/config/cache/MCP / 诊断安装、配置、缓存和 MCP
aicmd help me         # built-in help / 内置帮助
```

## 6. Built-in help / 内置帮助

Built-in help works before model/config initialization, so it is safe to use even when config is broken.

内置帮助在模型/配置初始化之前运行，所以即使配置损坏也可以安全使用。

```bash
aicmd help me
aicmd help setup
aicmd help search
aicmd help do
aicmd help session
aicmd help fix
aicmd help doctor
```

Chinese aliases are also supported for common topics, such as `aicmd help 配置`, `aicmd help 搜索`, and `aicmd help 修复`.

常见中文主题也支持，例如 `aicmd help 配置`、`aicmd help 搜索`、`aicmd help 修复`。

## 7. Regular command workflow / 普通命令工作流

Example:

示例：

```bash
aicmd 当前目录有多少文件
```

Before execution, AICmd shows:

执行前，AICmd 会显示：

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

Meaning:

含义：

```text
execute   run the generated command / 执行生成命令
revise    add correction and regenerate / 补充修改要求并重新生成
describe  explain the generated command / 解释命令
copy      copy generated command / 复制命令
quit      exit without execution / 不执行并退出
```

Useful flags:

常用参数：

```bash
aicmd --print 当前目录有多少文件       # print command only / 只打印命令
aicmd --dry-run 当前目录有多少文件     # preview prompt / 预览 prompt
aicmd --no-summary 当前目录有多少文件  # skip AI summary once / 本次跳过 AI summary
aicmd --summary 当前目录有多少文件     # force AI summary once / 本次强制 AI summary
aicmd --no-cache 当前目录有多少文件    # bypass successful command cache / 不复用缓存命令
```

## 8. Successful command cache / 成功命令缓存

AICmd stores successful regular commands in:

AICmd 会把成功执行过的普通命令缓存到：

```text
~/.aicmd/command-cache.yaml
```

When the same task, shell, and OS match later, AICmd may ask:

后续如果同一个任务、shell 和 OS 匹配，AICmd 可能会询问：

```text
Found a previously successful command / 找到一条之前成功执行过的命令:
reuse(复用) | new(重新生成) | describe(解释) | quit(退出):
```

Rules:

规则：

```text
- Only successful commands with exit code 0 are cached.
- 只有 exit code 为 0 的成功命令会缓存。
- do/search/err/revise flows are not cached.
- do/search/err/revise 流程不会缓存。
- Tasks with files via -f are not cached.
- 带 -f 文件输入的任务不会缓存。
- Sensitive tasks containing password/token/secret/api key/密钥/密码 are not cached.
- 包含 password/token/secret/api key/密钥/密码 的敏感任务不会缓存。
```

Bypass cache:

绕过缓存：

```bash
aicmd --no-cache 当前目录有多少文件
```

## 9. Failure repair loop / 失败修复循环

If a command exits with non-zero status, AICmd shows:

如果命令以非 0 状态退出，AICmd 会显示：

```text
fix(修复) | explain(解释) | copy(复制) | quit(退出):
```

Meaning:

含义：

```text
fix      generate a revised command from failure context / 根据失败上下文生成修复命令
explain  explain the failure / 解释失败原因
copy     copy the failed command / 复制失败命令
quit     exit / 退出
```

`fix` sends these fields to the model:

`fix` 会把这些信息发送给模型：

```text
original user task / 原始用户任务
shell / shell
OS / 操作系统
cwd / 当前目录
failed command / 失败命令
exit code / 退出码
stdout / 标准输出
stderr / 标准错误
```

The revised command is still shown for confirmation before execution. AICmd does not automatically run the fix.

修复命令仍会先展示并等待确认。AICmd 不会自动执行修复命令。

Automatic repair is limited to two attempts per command flow.

每条命令流程最多自动修复两次。

## 10. Script workflow: `aicmd do` / 脚本任务：`aicmd do`

Use `do` for multi-step tasks, file processing, and installation flows:

多步骤、文件处理、安装流程适合使用 `do`：

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --plan "安装 Docker"
aicmd do --dry-run "统计 logs/*.log 的 ERROR"
aicmd do -f notes.txt "按说明执行"
aicmd do --from-search gemini-cli "安装 gemini-cli"
aicmd do -o scripts/task.sh "清洗 CSV"
```

`--from-search` reads `~/.aicmd/searches/<name>.txt` and includes current system environment such as OS, architecture, cwd, and availability of `brew/node/npm/git/curl`.

`--from-search` 会读取 `~/.aicmd/searches/<name>.txt`，并附带当前系统环境，例如 OS、架构、当前目录、`brew/node/npm/git/curl` 是否存在。

## 11. Search and MCP / 搜索和 MCP

MCP config file:

MCP 配置文件：

```text
~/.aicmd/mcp.json
```

Minimal search example:

最小搜索示例：

```json
{
  "mcp": {
    "servers": {
      "tavily": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "tavily-mcp"],
        "env": {
          "TAVILY_API_KEY": "tvly-xxxx"
        }
      }
    },
    "commands": {
      "search": {
        "description": "Search the web using Tavily",
        "server": "tavily"
      }
    }
  }
}
```

Search commands:

搜索命令：

```bash
aicmd search "今天 AI 新闻"
aicmd search "gemini-cli 官方安装方式" --save gemini-cli
aicmd search list
aicmd search show gemini-cli
aicmd search open gemini-cli
aicmd search rm gemini-cli
aicmd search summarize last
```

After an interactive search:

交互式搜索完成后：

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

`aicmd mcp-raw <command> ...` prints raw MCP output and is mainly for debugging.

`aicmd mcp-raw <command> ...` 会打印 MCP 原始输出，主要用于调试。

## 12. Sessions / 会话

AICmd uses a daily session by default, such as `cmd-20260619`.

AICmd 默认使用每日会话，例如 `cmd-20260619`。

```bash
aicmd -s                       # show current/default session / 显示当前默认会话
aicmd -s dev                   # start or join dev session / 进入或创建 dev 会话
aicmd -s dev hello             # send request in dev session / 在 dev 会话中发送请求
aicmd --list-sessions          # list sessions / 列出会话
aicmd -s dev --empty-session   # clear dev session with confirmation / 二次确认后清空 dev 会话
```

History commands:

历史命令：

```bash
aicmd session
aicmd session list
aicmd session show
aicmd session show dev --limit 5
aicmd last
```

## 13. Config commands / 配置命令

```bash
aicmd config status          # safe status without secrets / 安全查看状态，不显示密钥
aicmd config init            # generate config.yaml from .env / 从 .env 生成配置
aicmd config init --force    # overwrite config with confirmation / 二次确认后覆盖配置
aicmd config path            # print config path / 输出配置路径
aicmd config dir             # print config dir / 输出配置目录
aicmd config show            # print config.yaml; may contain API keys / 输出配置，可能包含密钥
aicmd config edit            # edit config.yaml / 编辑配置
aicmd config summary status  # show AI summary default / 查看 AI summary 默认状态
aicmd config summary off     # disable AI summary by default / 默认关闭 AI summary
aicmd config summary on      # enable AI summary by default / 默认开启 AI summary
aicmd config mcp             # print mcp.json path / 输出 mcp.json 路径
aicmd config doctor          # same as aicmd doctor / 等同于 aicmd doctor
```

`aicmd config status` prints safe fields only:

`aicmd config status` 只输出安全字段：

```text
config file / 配置文件
default model / 默认模型
temperature / 温度
AI summary / AI 总结
MCP config / MCP 配置
Search / 搜索
Session / 会话
```

## 14. Error diagnosis: `aicmd err` / 报错诊断：`aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

`aicmd err` really runs the command, captures stdout/stderr/exit code, and asks the model to generate diagnostic or fix commands for your review.

`aicmd err` 会真实执行命令，捕获 stdout/stderr/exit code，然后让模型生成诊断或修复命令供你检查。

## 15. Shell integration / Shell 集成

Shell integration lets commands such as `cd ..` affect the current terminal. Normal installs usually configure it automatically. If needed:

Shell 集成用于让 `cd ..` 这类命令影响当前终端。正常安装通常会自动配置；如有需要：

```bash
eval "$(aicmd shell-init)"
```

Without shell integration, AICmd can still run commands, but directory changes do not persist in the current shell.

没有 shell 集成时，AICmd 仍能执行命令，但目录变化不会保留在当前 shell。

## 16. Update / 更新

```bash
aicmd update --check
aicmd update
aicmd update --version v0.4.2
aicmd update --dry-run
```

## 17. Unsupported upstream AIChat options / 不支持的上游 AIChat 选项

The following broad AIChat-style workflows are not part of AICmd's public CLI:

以下 AIChat 风格的宽功能不属于 AICmd 公开 CLI：

```text
-e / --execute
-r / --role
-c / --code
--prompt
--agent
--rag
--macro
--serve
--sync-models
--list-models
--list-roles
--list-agents
--list-rags
--list-macros
--info
-S / --no-stream
```

Use plain AICmd instead of `-e`:

不要使用 `-e`，直接使用 AICmd：

```bash
# unsupported old style / 不支持的旧方式
aicmd -e 当前目录下有多少文件

# current style / 当前方式
aicmd 当前目录下有多少文件
```

## 18. Safety model / 安全模型

AICmd keeps a human in the loop:

AICmd 保持人参与执行决策：

```text
- It shows the generated command before execution.
- 执行前展示生成命令。
- It asks for confirmation before running.
- 执行前要求确认。
- It shows risk labels for generated commands.
- 显示命令风险等级。
- Destructive commands require extra confirmation.
- 破坏性命令需要额外确认。
- Fix commands after failure also require confirmation.
- 失败后的修复命令也需要确认。
```

Recommended habits:

建议习惯：

```text
- Read the generated command before pressing execute.
- 按 execute 前先阅读生成命令。
- Use --dry-run when checking prompts.
- 检查 prompt 时使用 --dry-run。
- Use --no-cache when you want a fresh command.
- 想重新生成命令时使用 --no-cache。
- Be explicit when files may be modified or deleted.
- 如果可能修改或删除文件，请明确说明限制。
```

## 19. Troubleshooting / 排障

`aicmd doctor` performs offline checks for binary path, version, config, model, temperature, AI summary, MCP/search, command cache, saved searches directory, PATH, and shell integration.

`aicmd doctor` 会执行离线检查，包括二进制路径、版本、配置、模型、temperature、AI summary、MCP/search、命令缓存、搜索记录目录、PATH 和 shell 集成。

Check installation:

检查安装：

```bash
which aicmd
aicmd --version
aicmd doctor
aicmd config status
```

If `aicmd` is not found:

如果找不到 `aicmd`：

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

If config is missing:

如果配置缺失：

```bash
aicmd setup
# or / 或者
aicmd init --from-env
```

If `.env` changed but model did not change:

如果修改 `.env` 后模型没有变化：

```bash
aicmd init --from-env --force
```

If MCP search times out:

如果 MCP 搜索超时：

```bash
AICMD_MCP_START_TIMEOUT_SECS=300 AICMD_MCP_CALL_TIMEOUT_SECS=600 aicmd search "今天北京天气"
```

If `cd ..` runs but current directory does not change:

如果 `cd ..` 执行后当前目录没有变化：

```bash
eval "$(aicmd shell-init)"
```

## 20. Quick reference / 快速参考

```bash
# Basic command generation / 基础命令生成
aicmd 当前目录下有多少文件

# Force fresh generation / 强制重新生成
aicmd --no-cache 当前目录下有多少文件

# Print command only / 只打印命令
aicmd --print 当前目录下有多少文件

# Complex script workflow / 复杂脚本工作流
aicmd do "清洗 input.csv，输出 cleaned.csv"

# Search / 搜索
aicmd search "copilot-cli 如何安装"

# Use search result as execution context / 基于搜索结果执行
aicmd do --from-search last "安装 copilot-cli"

# Diagnose failing command / 诊断失败命令
aicmd err -- pnpm test

# Config status / 配置状态
aicmd config status

# Doctor / 诊断
aicmd doctor
```
