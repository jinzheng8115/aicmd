# AICmd Usage Guide / AICmd 使用文档

This document describes the current AICmd command-line workflow.

本文档说明当前版本 AICmd 的命令行使用方式。

## 1. What AICmd does / AICmd 是什么

English:
AICmd turns natural language into shell commands. You describe a terminal task, AICmd generates a command, then you review it before execution.

中文：
AICmd 把自然语言转换成 shell 命令。你描述一个终端任务，AICmd 生成命令，然后你确认后再执行。

AICmd is intentionally focused. It does not expose the broad upstream AIChat workflows such as Chat REPL, RAG, agents, macros, public role switching, custom tools/functions, or built-in server mode.

AICmd 刻意保持聚焦。它不暴露上游 AIChat 的宽功能，例如 Chat REPL、RAG、agents、macros、公开 role 切换、自定义 tools/functions、内置 server mode。

## 2. Install / 安装

AICmd supports macOS, Linux, Windows PowerShell, and Windows WSL.

AICmd 支持 macOS、Linux、Windows PowerShell 和 Windows WSL。

### Release binary install, no Rust required / Release 二进制安装，不需要 Rust

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Windows PowerShell:

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
```

### Source/developer install, Rust required / 源码或开发安装，需要 Rust

From the repository root:

在项目根目录执行：

```bash
cp .env.example .env
$EDITOR .env
contrib/aicmd/install.sh --from-source
aicmd init --from-env
```

The installer puts `aicmd` in the user bin directory and creates compatibility wrappers:

安装器会把 `aicmd` 放到用户 bin 目录，并创建兼容 wrapper：

```text
aicmd-do      -> aicmd do
aicmd-err     -> aicmd err
aicmd-model   -> aicmd model
aicmd-mcp     -> aicmd mcp-raw
aicmd-shell-init -> aicmd shell-init
```

Make sure the install directory is in your `PATH`.

请确认安装目录已加入 `PATH`。

If your shell has cached an older `aicmd`, refresh the command hash:

如果 shell 缓存了旧版 `aicmd`，刷新命令缓存：

```bash
hash -r
```

Verify the installed binary:

验证安装结果：

```bash
which aicmd
file $(which aicmd)
aicmd --help
```

Expected: `file` should report a native executable, not a shell script.

期望结果：`file` 应显示原生可执行文件，而不是 shell script。

## 2.2 MCP tools / MCP 工具

MCP tools stay outside the normal terminal-command generation loop. Configure MCP servers and command mappings in `mcp.json`; the installer copies it to `~/.aicmd/mcp.json` if that file does not already exist. User-facing shortcuts such as `aicmd search <query>` and `aicmd mcp <command> ...` call MCP first and then use the configured LLM to summarize the result. The lower-level `aicmd mcp-raw <command> ...` command prints raw MCP output for debugging. `aicmd-mcp` remains as a compatibility wrapper.

MCP 工具不进入普通终端命令生成循环。MCP server 和命令映射配置在 `mcp.json`；安装脚本会在 `~/.aicmd/mcp.json` 不存在时复制过去。面向用户的 `aicmd search <query>` 和 `aicmd mcp <command> ...` 会先调用 MCP，再使用当前配置的 LLM 整理结果。底层 `aicmd mcp-raw <command> ...` 直接打印 MCP 原始输出，方便调试；`aicmd-mcp` 仅作为兼容 wrapper。

```bash
aicmd search "今天 AI 新闻"
aicmd search DeepSeek latest model

# Underlying helper / 底层辅助命令
aicmd mcp-raw search "今天 AI 新闻"
aicmd mcp-raw tavily "DeepSeek latest model"
```

Use `aicmd mcp list` to see configured MCP commands.

使用 `aicmd mcp list` 查看已配置的 MCP 命令。

## 2.1 Shell integration for `cd` / 用于 `cd` 的 Shell 集成

AICmd normally runs commands in a child process. A plain child process cannot change the parent terminal directory, so commands like `cd ..` need shell integration.

AICmd 默认在子进程中执行命令。普通子进程不能改变父终端目录，所以 `cd ..` 这类命令需要 shell 集成。

Enable it in the current shell:

在当前 shell 中启用：

```bash
eval "$(aicmd shell-init)"
```

Enable it permanently for zsh:

在 zsh 中永久启用：

```bash
echo 'eval "$(aicmd shell-init)"' >> ~/.zshrc
```

After this, if AICmd executes a command that changes directory, the wrapper updates your current shell directory after the command finishes.

启用后，如果 AICmd 执行了切换目录的命令，wrapper 会在命令结束后同步更新当前 shell 目录。

## 3. Basic workflow / 基础工作流

Run AICmd with a natural-language task:

用自然语言任务调用 AICmd：

```bash
aicmd 当前目录下有多少文件
```

AICmd generates a shell command and asks what to do next.

AICmd 会生成 shell 命令，并询问下一步操作。

Interactive choices:

交互选项：

```text
execute | revise | describe | copy | quit
```

Meaning:

含义：

```text
execute   run the generated command / 执行生成的命令
revise    add a correction and regenerate / 补充修改要求并重新生成
describe  explain the generated command / 解释生成的命令
copy      copy the generated command to clipboard / 复制生成的命令到剪贴板
quit      exit without executing / 不执行并退出
```

AICmd does not execute the generated command automatically by default. When you choose `execute`, AICmd prints the raw command output first, then summarizes the result with the configured LLM.

AICmd 默认不会自动执行生成的命令。选择 `execute` 后，AICmd 会先打印命令原始输出，再用当前配置的 LLM 总结执行结果。

## 4. Current CLI options / 当前 CLI 参数

Current help output:

当前帮助信息：

```text
Usage: aicmd [OPTIONS] [TEXT]...

Arguments:
  [TEXT]...  Input text

Options:
  -m, --model <MODEL>        Select a LLM model
  -s, --session [<SESSION>]  Start or join a session
      --empty-session        Ensure the session is empty
  -f, --file <FILE>          Include files, directories, or URLs
      --dry-run              Display the message without sending it
      --list-sessions        List all sessions
  -h, --help                 Print help
  -V, --version              Print version
```

## 5. Common examples / 常用示例

List large files:

列出大文件：

```bash
aicmd 列出当前目录最大的 10 个文件
```

Count files in the current directory:

统计当前目录文件数：

```bash
aicmd 当前目录下有多少文件
```

Compress images:

压缩图片：

```bash
aicmd 把当前目录下的 png 图片压缩到 dist/images
```

Run tests and suggest a safe command sequence:

运行测试并生成安全命令：

```bash
aicmd 运行测试并修复明显问题
```

Use a specific model:

指定模型：

```bash
aicmd -m openai:gpt-4o 当前目录下有多少文件
```

Preview the prompt/messages without calling the model:

只预览发送内容，不调用模型：

```bash
aicmd --dry-run 当前目录下有多少文件
```

## 6. Sessions / 会话

AICmd uses command sessions so related command-generation context can continue. Sessions store conversation context, not the selected model. The model always comes from current `~/.aicmd/config.yaml` or the current `-m` option.

AICmd 使用命令会话，以便相关命令生成上下文可以延续。会话只保存对话上下文，不保存所选模型。模型始终来自当前 `~/.aicmd/config.yaml` 或本次命令的 `-m` 参数。

Show the current default session name:

显示当前默认会话名称：

```bash
aicmd -s
```

Default session:

默认会话：

```text
cmd-YYYYMMDD
```

Example on 2026-06-16:

例如 2026-06-16：

```text
cmd-20260616
```

Create or open a named session without sending a request:

只创建或打开指定会话，不发送请求：

```bash
aicmd -s dev
```

Create or open a named session and send a request at the same time:

创建或打开指定会话，并同时发送请求：

```bash
aicmd -s dev 运行测试
```

Continue the same named session later:

之后继续同一个指定会话：

```bash
aicmd -s dev 修复刚才的测试失败
```

List sessions:

列出会话：

```bash
aicmd --list-sessions
```

Clear a session before use:

使用前清空会话：

```bash
aicmd -s dev --empty-session 重新开始分析当前目录
```

Notes:

注意：

```text
- If you do not pass -s, AICmd uses the daily session.
- If you pass -s <name>, AICmd uses that named session.
- --empty-session only applies to the selected session.
```

```text
- 不传 -s 时，AICmd 使用每日会话。
- 传入 -s <name> 时，AICmd 使用该指定会话。
- --empty-session 只作用于当前选择的会话。
```

## 7. File and URL input / 文件和 URL 输入

Use `-f` to include local files, directories, URLs, or supported command-style inputs.

使用 `-f` 引入本地文件、目录、URL 或受支持的命令式输入。


Examples:

示例：

```bash
aicmd -f README.md 总结这个项目的安装步骤
```

```bash
aicmd -f data.csv 统计每列空值数量
```

```bash
aicmd -f logs/ 提取最近的 ERROR 日志
```

Multiple inputs:

多个输入：

```bash
aicmd -f README.md -f Cargo.toml 说明这个项目如何构建
```

File loading can be customized through `document_loaders` in the config.

文件加载方式可以通过配置文件里的 `document_loaders` 自定义。

## 8. Model configuration / 模型配置

AICmd uses a visible config directory by default:

AICmd 默认使用一个用户可见的配置目录：

```text
~/.aicmd
```

Recommended setup order:

推荐设置顺序：

```bash
# 1. Copy the simple env file / 复制简单配置文件
cp .env.example .env

# 2. Fill model and MCP values / 填写模型和 MCP 参数
$EDITOR .env

# 3. Install commands / 安装命令
contrib/aicmd/install.sh

# 4. Generate ~/.aicmd/config.yaml with confirmation / 二次确认后生成 ~/.aicmd/config.yaml
aicmd init --from-env
# Equivalent helper / 等价辅助命令：aicmd init --from-env

# 5. Test / 测试
aicmd 当前目录下有多少文件
```

The pre-install `.env` only asks for one provider configuration with these values:

安装前的 `.env` 只需要填写一组模型服务配置，包含这些值：

```text
AICMD_MODEL_NAME       model display/client name / 模型标识
AICMD_MODEL_PROVIDER   openai | anthropic | google / 接口种类
AICMD_MODEL_API_BASE   API base URL / API 地址
AICMD_MODEL_API_KEY    API key
AICMD_MODEL_IDS        provider model id(s), comma-separated / 模型 ID，多个用逗号分隔
AICMD_DEFAULT_MODEL    optional default model; default is name:first-id / 可选默认模型；默认是 name:第一个模型 ID
AICMD_OPENAI_API_STYLE openai only: chat | responses / 仅 openai 需要
MCP is configured in mcp.json, not .env / MCP 在 mcp.json 中配置，不在 .env 中配置
```


MCP config file example:

MCP 配置文件示例：

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
      },
      "context7": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "@upstash/context7-mcp"]
      }
    },
    "commands": {
      "search": {
        "description": "Search the web using Tavily",
        "server": "tavily"
      },
      "context7-library": {
        "description": "Resolve a package/library name using Context7",
        "server": "context7"
      },
      "tavily": {
        "description": "Alias of search using Tavily",
        "server": "tavily"
      }
    }
  }
}
```

Command mapping rules:

命令映射规则：

```text
mcp.servers.<server-name>    defines how to start the MCP server / 定义如何启动 MCP server
mcp.commands.<command>.server selects a configured server / 选择一个已配置 server
```


Examples:

示例：

```bash
aicmd search "OpenAI latest news"
aicmd mcp-raw search "OpenAI latest news"
aicmd mcp context7-library react
```

Use `aicmd mcp list` to see configured MCP commands.

使用 `aicmd mcp list` 查看已配置的 MCP 命令。

When `.env` is converted to `config.yaml`, `AICMD_MODEL_IDS=gpt-4o,gpt-4.1` becomes multiple entries under `clients[].models`. The generated top-level `model:` is `AICMD_DEFAULT_MODEL` when set; otherwise it is `AICMD_MODEL_NAME:first-id-in-AICMD_MODEL_IDS`.

`.env` 转成 `config.yaml` 时，`AICMD_MODEL_IDS=gpt-4o,gpt-4.1` 会变成 `clients[].models` 下的多个模型。顶部 `model:` 如果设置了 `AICMD_DEFAULT_MODEL` 就使用它；否则使用 `AICMD_MODEL_NAME:AICMD_MODEL_IDS 中的第一个模型 ID`。

After installation, the user-editable runtime config file is:

安装后，用户可编辑的运行时配置文件是：

```text
~/.aicmd/config.yaml  # LLM and MCP config / LLM 和 MCP 配置
```

AICmd no longer ships a separate public model template or `models.yaml`. Add or switch models directly in runtime `~/.aicmd/config.yaml`. MCP reads `~/.aicmd/mcp.json` directly. To switch provider through `.env`, edit the same `.env` file and regenerate with `aicmd init --from-env --force`. `aicmd model init` asks for confirmation before writing config; `aicmd-model` is a compatibility wrapper.

AICmd 不再提供单独的公开模型模板或 `models.yaml`。新增或切换模型时，可以直接编辑运行时 `~/.aicmd/config.yaml`。MCP 直接读取 `~/.aicmd/mcp.json`。如果通过 `.env` 切换服务商，请修改同一个 `.env` 文件后用 `aicmd init --from-env --force` 重新生成。`aicmd model init` 写入配置前会二次确认；`aicmd-model` 是兼容 wrapper。

Important config fields:

重要配置字段：

```yaml
model: openai:gpt-4o
temperature: null
top_p: null
stream: false
save: true
wrap: no
highlight: true
save_shell_history: true
```

Client example:

模型服务示例：

```yaml
clients:
  - type: openai
    api_base: https://api.openai.com/v1
    api_key: xxx
    api_style: chat  # or responses / 或 responses
    models:
      - name: gpt-4o
```

OpenAI-compatible example:

OpenAI-compatible 示例：

```yaml
clients:
  - type: openai-compatible
    name: ollama
    api_base: http://localhost:11434/v1
    api_key: xxx
    models:
      - name: llama3.1
        max_input_tokens: 128000
```

Explicit config directory override:

显式指定配置目录：

```bash
export AICMD_CONFIG_DIR=/path/to/aicmd-config
```

Other explicit overrides also follow the `AICMD_...` environment naming pattern.

其他显式覆盖项也遵循 `AICMD_...` 环境变量命名方式。


## 9. Model command: aicmd model / 模型命令：aicmd model

`aicmd model` helps users find, show, and edit runtime config. `aicmd init --from-env` reads the simple `.env` file and writes LLM config to `~/.aicmd/config.yaml`. MCP config lives in `~/.aicmd/mcp.json`.

`aicmd model` 用于创建、定位、查看和编辑运行时配置。`aicmd init --from-env` 会读取简单 `.env` 文件，把 LLM 配置写入 `~/.aicmd/config.yaml`。MCP 配置保存在 `~/.aicmd/mcp.json`。

Usage:

用法：

```bash
aicmd init --from-env
# Equivalent helper / 等价辅助命令：aicmd init --from-env
aicmd model path
aicmd model show
EDITOR=vim aicmd model edit
```

Typical flow after installation if `.env` was not used:

如果安装时没有使用 `.env`，安装后的典型流程：

```bash
aicmd init --from-env
# or edit config directly / 或直接编辑配置
EDITOR=vim aicmd model edit
```

Typical flow to add another model later:

之后新增其他模型的典型流程：

```bash
EDITOR=vim aicmd model edit
```

Then add the provider/model under `clients` and update the top-level `model` field if you want it to become the default.

然后在 `clients` 下新增 provider/model；如果希望它成为默认模型，再修改顶部的 `model` 字段。

## 10. Legacy AIChat config migration / 旧 AIChat 配置迁移

AICmd is derived from AIChat but now defaults to the visible `~/.aicmd` config directory.

AICmd 基于 AIChat 改造，但现在默认使用用户可见的 `~/.aicmd` 配置目录。

On first startup, if no `~/.aicmd` config exists and an older AICmd or AIChat config exists, AICmd copies only these files into the new directory:

首次启动时，如果 `~/.aicmd` 配置不存在但旧版 AICmd 或 AIChat 配置存在，AICmd 只复制这些文件到新目录：

```text
config.yaml
.env
```

AICmd intentionally does not migrate broad upstream workflow data such as:

AICmd 刻意不迁移这些上游宽功能数据：

```text
messages.md
roles/
functions/
agent configs
```

This keeps the new project focused while preserving provider credentials and basic model configuration.

这样可以保留 provider 凭据和基础模型配置，同时让新项目保持聚焦。

## 11. Script command: aicmd do / 脚本命令：aicmd do

`aicmd do` asks AICmd to generate commands that create a local task script, make it runnable, and run it through AICmd's normal confirmation flow. `aicmd-do` remains as a compatibility wrapper.

`aicmd do` 会让 AICmd 生成命令：创建本地任务脚本、设置为可运行，并通过 AICmd 正常确认流程运行；`aicmd-do` 仅作为兼容 wrapper。

Usage:

用法：

```bash
aicmd do [--dry-run] [--output PATH] <task>
```

Examples:

示例：

```bash
aicmd do "统计 data.csv 每列的空值数量"
```

```bash
aicmd do --output scripts/clean-data.sh "清洗 input.csv，输出 cleaned.csv"
```

```bash
aicmd do --dry-run "处理 logs/*.log，提取 ERROR 行到 errors.txt"
```

Default script path:

默认脚本路径：

```text
.aicmd/task-YYYYMMDD-HHMMSS.sh
```

Safety behavior:

安全行为：

```text
- The generated script should check input files first.
- It should create output directories when needed.
- It should not delete or overwrite original files unless the task explicitly asks for that.
- You still review the generated command before execution.
```

```text
- 生成的脚本应先检查输入文件。
- 必要时创建输出目录。
- 除非任务明确要求，否则不应删除或覆盖原始文件。
- 执行前仍然需要你确认生成的命令。
```

## 12. Error command: aicmd err / 报错命令：aicmd err

`aicmd err` runs a command, captures stdout, stderr, and exit code, then asks AICmd to generate diagnostic or fix commands. `aicmd-err` remains as a compatibility wrapper.

`aicmd err` 会运行一条命令，捕获 stdout、stderr 和 exit code，然后让 AICmd 生成诊断或修复命令；`aicmd-err` 仅作为兼容 wrapper。

Usage:

用法：

```bash
aicmd err -- <command> [args...]
aicmd err <command> [args...]
```

Examples:

示例：

```bash
aicmd err -- pnpm test
```

```bash
aicmd err -- cargo test
```

```bash
aicmd err -- npm run build
```

`aicmd err` does not directly apply fixes. It sends the captured failure context to AICmd, which then generates shell commands for you to review.

`aicmd err` 不会直接应用修复。它把失败上下文发送给 AICmd，由 AICmd 生成 shell 命令，再由你确认。

## 13. Removed or unsupported upstream options / 已移除或不支持的上游选项

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

If you pass these flags to current AICmd, they should be rejected as unexpected arguments.

如果你把这些参数传给当前 AICmd，它们应被作为 unexpected arguments 拒绝。

Use plain AICmd instead of `-e`:

不要使用 `-e`，直接使用 AICmd：

```bash
# old style, unsupported / 旧方式，不支持
aicmd -e 当前目录下有多少文件

# current style / 当前方式
aicmd 当前目录下有多少文件
```

## 14. Safety model / 安全模型

AICmd is designed to keep a human in the loop.

AICmd 的设计目标是让人保持在执行环节中。

Default safety behavior:

默认安全行为：

```text
- Generate a shell command from the task.
- Show the generated command.
- Ask before execution.
- Let the user revise, describe, copy, execute, or quit.
```

```text
- 根据任务生成 shell 命令。
- 展示生成的命令。
- 执行前询问确认。
- 用户可以修改、解释、复制、执行或退出。
```

Recommended habits:

建议习惯：

```text
- Read the generated command before pressing execute.
- Use --dry-run when changing prompts, sessions, or config.
- Use a named session for a project-specific workflow.
- Be explicit when a command may modify or delete files.
```

```text
- 按 execute 前先阅读生成的命令。
- 调整 prompt、session 或配置时先用 --dry-run。
- 项目相关工作建议使用命名 session。
- 如果命令可能修改或删除文件，请明确说明限制。
```

## 15. Troubleshooting / 排障

Check which AICmd is running:

检查当前运行的是哪个 AICmd：

```bash
which aicmd
file $(which aicmd)
```

Expected installed binary:

期望的安装结果：

```text
Mach-O 64-bit executable arm64      # macOS Apple Silicon example
ELF ... executable                  # Linux example
```

If it says shell script, you may still be running an old wrapper. Reinstall and refresh the shell command hash:

如果显示 shell script，说明你可能还在运行旧 wrapper。重新安装并刷新 shell 缓存：

```bash
contrib/aicmd/install.sh
hash -r
```

Show help:

查看帮助：

```bash
aicmd --help
```

List sessions:

列出会话：

```bash
aicmd --list-sessions
```

Preview without model call:

不调用模型，只预览发送内容：

```bash
aicmd --dry-run 当前目录下有多少文件
```

Use an explicit config directory:

使用显式配置目录：

```bash
AICMD_CONFIG_DIR=/path/to/config aicmd --help
```

## 16. Quick reference / 快速参考

```bash
# Basic command generation / 基础命令生成
aicmd 当前目录下有多少文件

# Named session / 命名会话
aicmd -s dev 运行测试

# Clear selected session / 清空所选会话
aicmd -s dev --empty-session 重新分析当前项目

# Include files / 引入文件
aicmd -f README.md 总结安装步骤

# Preview messages / 预览消息
aicmd --dry-run 当前目录下有多少文件

# Generate a script workflow / 生成脚本工作流
aicmd do "清洗 input.csv，输出 cleaned.csv"

# Analyze a failing command / 分析失败命令
aicmd err -- pnpm test

# Show or edit model config / 查看或编辑模型配置
aicmd model show
EDITOR=vim aicmd model edit
```
