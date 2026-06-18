# AICmd

[English](README.en.md)

AICmd 用自然语言生成可检查、可确认的终端命令。你描述要做什么，AICmd 生成 shell 命令；你确认后执行，执行完成后还可以由 LLM 总结命令输出。

上游来源：[sigoden/aichat](https://github.com/sigoden/aichat)
许可证：MIT OR Apache-2.0

## 1. AICmd 适合做什么

- 用自然语言生成终端命令。
- 为本地文件/数据处理任务生成脚本并执行。
- 捕获报错命令的输出，让 LLM 生成诊断/修复命令。
- 调用已配置的 MCP 工具，例如 web search，再让 LLM 整理结果。
- 项目聚焦“自然语言运行终端命令”，不追求通用聊天应用的全部功能。

## 2. 平台支持

已发布的二进制平台：

| 系统 | CPU / 架构 | Release target |
| --- | --- | --- |
| macOS Apple Silicon | arm64 / aarch64 | `aarch64-apple-darwin` |
| macOS Intel | x86_64 | `x86_64-apple-darwin` |
| Linux ARM64 | arm64 / aarch64 | `aarch64-unknown-linux-musl` |
| Linux Intel/AMD | x86_64 | `x86_64-unknown-linux-musl` |
| Windows ARM64 | arm64 / aarch64 | `aarch64-pc-windows-msvc` |
| Windows Intel/AMD | x86_64 | `x86_64-pc-windows-msvc` |

Windows WSL 可以使用 Linux 安装方式。

## 3. 安装前准备

### 3.1 必填：模型 `.env`

AICmd 需要一组 LLM 模型配置。推荐先准备 `.env`，再由它生成 `config.yaml`。

最小 OpenAI 兼容示例：

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
```

支持的模型接口：

| 接口 | `AICMD_MODEL_PROVIDER` | 说明 |
| --- | --- | --- |
| OpenAI 或兼容接口 | `openai` | 需要 `AICMD_OPENAI_API_STYLE=chat` 或 `responses` |
| Anthropic Claude | `anthropic` | 写入 config 时使用 Claude client |
| Google Gemini | `google` | 写入 config 时使用 Gemini client |

常用 `.env` 字段：

```env
# 显示名/客户端名，会写入 config.yaml。
AICMD_MODEL_NAME=openai

# openai | anthropic | google
AICMD_MODEL_PROVIDER=openai

# 模型 API 地址。
AICMD_MODEL_API_BASE=https://api.openai.com/v1

# 模型 API Key。
AICMD_MODEL_API_KEY=sk-xxxx

# 一个或多个模型 ID，用英文逗号分隔。
AICMD_MODEL_IDS=gpt-4o,gpt-4.1

# 可选默认模型。不填时使用 MODEL_NAME:第一个 MODEL_ID。
# AICMD_DEFAULT_MODEL=openai:gpt-4o

# 仅 OpenAI 或兼容接口需要：chat | responses
AICMD_OPENAI_API_STYLE=chat
```

如果你 clone 了仓库，也可以复制模板：

```bash
cp .env.example .env
$EDITOR .env
```

如果你使用一行命令二进制安装、没有 clone 仓库，请根据上面的字段自己创建一个本地 `.env` 文件。后续在这个 `.env` 所在目录运行 `aicmd init --from-env`，或设置 `AICMD_MODEL_ENV=/path/to/.env`。

### 3.2 可选：MCP `mcp.json`

MCP 与模型配置分开。运行时 MCP 文件是：

```text
~/.aicmd/mcp.json
```

如果该文件不存在，安装器会创建一个 starter `mcp.json`。推荐做法是把准备好的 `.env` 和 `mcp.json` 放在同一个目录，然后运行 `aicmd init --from-env`；AICmd 会根据 `.env` 生成 `~/.aicmd/config.yaml`，并把 `mcp.json` 复制到 `~/.aicmd/mcp.json`。

手动创建或编辑：

```bash
mkdir -p ~/.aicmd
$EDITOR ~/.aicmd/mcp.json
```

示例：

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
      }
    }
  }
}
```

说明：
- `servers` 定义如何启动 MCP server。
- `commands` 定义用户在 AICmd 里输入的命令名。
- MCP 命令会先拿到工具原始结果，再交给当前 LLM 整理成适合终端阅读的输出。

## 4. 安装

### 4.1 推荐：二进制安装，不需要 Rust

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 | iex
```

注意：不要使用 `iwr ... | iex`。`iwr` / `Invoke-WebRequest` 返回的是响应对象，不是脚本文本；如果必须使用 `iwr`，请写成 `(iwr URL -UseBasicParsing).Content | iex`。

默认安装位置：

| 内容 | macOS / Linux | Windows |
| --- | --- | --- |
| 主程序 | `~/.local/bin/aicmd` | `~/.local/bin/aicmd.exe` |
| 运行时配置 | `~/.aicmd/config.yaml` | `~/.aicmd/config.yaml` |
| MCP 配置 | `~/.aicmd/mcp.json` | `~/.aicmd/mcp.json` |

安装器还会为旧脚本创建兼容 wrapper，例如 `aicmd-do`、`aicmd-err`、`aicmd-model`、`aicmd-shell-init`。新用法建议直接参考下面的 `aicmd` 主命令示例。

### 4.2 源码或开发安装，需要 Rust

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
cp .env.example .env
$EDITOR .env
contrib/aicmd/install.sh --from-source
```

## 5. 安装后：生成 `config.yaml`

安装二进制后，用 `.env` 生成运行时模型配置。如果同目录存在 `mcp.json`，会同时复制到 `~/.aicmd/mcp.json`。

```bash
aicmd init --from-env
```

这会写入或更新：

```text
~/.aicmd/config.yaml
~/.aicmd/mcp.json   # 仅当本地 mcp.json 存在时
```

AICmd 写入前会二次确认。这是有意设计，因为 `config.yaml` 会包含你的 API key。

如果 `.env` 不在当前目录，可以显式指定。AICmd 也会查找这个 `.env` 同目录下的 `mcp.json`；也可以用 `AICMD_MCP_SOURCE=/path/to/mcp.json` 指定 MCP 来源文件。

```bash
AICMD_MODEL_ENV=/path/to/.env aicmd init --from-env
AICMD_MODEL_ENV=/path/to/.env AICMD_MCP_SOURCE=/path/to/mcp.json aicmd init --from-env
```

如果 `config.yaml` 已存在并且你要重新生成：

```bash
aicmd init --from-env --force
```

常用检查：

```bash
aicmd model path      # 查看 config.yaml 路径
aicmd model show      # 输出 config.yaml
aicmd model edit      # 编辑 config.yaml
```

## 6. Shell 集成

Shell 集成的作用是：当 AICmd 执行 `cd ..` 这类命令后，让当前终端目录也真的跟着变化。

正常首次安装后，不需要手动执行 shell integration 命令。

安装器行为：
- macOS / Linux 安装器会自动写入 `~/.zshrc` 或 `~/.bashrc`。
- Windows 安装器会自动写入 PowerShell profile。
- 安装完成后，新开一个终端或 PowerShell 窗口即可自动生效。

手动启用只用于特殊情况：
- 安装时使用了 `--no-shell-integration` 或 `-NoProfile`。
- 不想新开终端，希望当前已经打开的终端立即生效。

手动命令：

```bash
# zsh / bash
eval "$(aicmd shell-init)"
```

```powershell
# PowerShell
Invoke-Expression ((& aicmd shell-init powershell) -join [Environment]::NewLine)
```

如果禁用了 shell 集成，AICmd 仍然可以执行命令，但 `cd` 的结果不会同步到当前终端目录。

## 7. 命令用法

### 7.1 基础自然语言命令

```bash
aicmd 当前目录有多少文件
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
```

AICmd 会显示生成的命令并询问：

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

选项含义：
- `execute` / `e`：执行命令
- `revise` / `r`：让 LLM 修改命令
- `describe` / `d`：用中文解释命令
- `copy` / `c`：复制命令
- `quit` / `q`：退出，不执行

执行后，AICmd 会先输出原始命令结果，再让 LLM 做 summary。命令、退出码、截断后的 stdout/stderr 和 summary 会写入当前 session，方便下一轮继续引用“刚才的执行结果”。

### 7.2 全局系统参数

这些参数属于 `aicmd` 系统本身，用来控制 AICmd 的运行方式，不属于自然语言任务内容。

| 参数 | 含义 | 示例 |
| --- | --- | --- |
| `-m, --model <MODEL>` | 本次请求临时使用指定模型。 | `aicmd -m openai:gpt-4o 当前目录有多少文件` |
| `-s, --session [SESSION]` | 不带名称时显示当前 session；带名称时进入或创建指定 session。 | `aicmd -s`, `aicmd -s dev hello` |
| `--empty-session` | 清空并重建所选 session，会二次确认。 | `aicmd -s dev --empty-session` |
| `-f, --file <FILE>` | 把文件、目录或 URL 作为上下文传给模型。 | `aicmd -f README.md 总结这个文件` |
| `--dry-run` | 只显示将要发送的内容，不调用 LLM。用于调试 prompt、session、配置行为。 | `aicmd --dry-run 当前目录有多少文件` |
| `--list-sessions` | 列出已保存的 session。 | `aicmd --list-sessions` |
| `-h, --help` | 显示帮助。 | `aicmd --help` |
| `-V, --version` | 显示版本。 | `aicmd --version` |

当前 CLI 有 8 个全局系统参数。

子命令也有自己的参数：

| 命令 | 参数 | 含义 |
| --- | --- | --- |
| `aicmd do` | `--dry-run` | 生成脚本任务请求但不发送给 LLM。 |
| `aicmd do` | `--plan` | 只生成执行计划，不创建或运行任务脚本。 |
| `aicmd do` | `-f, --file <FILE>` | 把保存的文本文件作为任务上下文，例如之前的搜索结果。 |
| `aicmd do` | `--from-search <NAME>` | 直接引用已保存搜索结果，例如 `~/.aicmd/searches/<NAME>.txt`。 |
| `aicmd do` | `-o, --output <PATH>` | 指定生成脚本路径。 |
| `aicmd model init` / `aicmd init` | `--from-env` | 必须读取 `.env` 并生成 `~/.aicmd/config.yaml`。 |
| `aicmd model init` / `aicmd init` | `--force` | 覆盖已有 `config.yaml`，会二次确认。 |
| `aicmd shell-init` | `zsh`, `bash`, `powershell` | 输出对应 shell 的集成代码；正常安装后通常不需要手动执行。 |
| `aicmd doctor` | 无 | 检查安装、模型配置、MCP/search、PATH 和 shell 集成状态。 |
| `aicmd session` | `list`, `show`, `--limit` | 查看当前会话、会话列表和最近消息。 |
| `aicmd last` | 无 | 查看当前默认会话最后一条非 system 消息。 |
| `aicmd search` | `--save [name]` | 搜索后立即保存结果；不写 name 时自动生成名称。 |
| `aicmd search` | `save [name]` | 如果先搜索、满意后再保存，则保存上一次搜索结果。 |
| `aicmd search` | `summarize <name|last>` | 重新整理已保存的 raw 搜索结果。 |
| `aicmd search` | `list`, `show <name|last>` | 列出或查看已保存搜索；列表会显示 summary/raw 状态。 |
| `aicmd search` | `open <name|last>`, `rm <name>` | 打开或删除已保存搜索记录。 |
| `aicmd update` | `--check`, `--version`, `--dry-run` | 检查或使用官方安装器更新 AICmd。 |

### 7.3 会话

```bash
aicmd -s                 # 显示当前默认 session
aicmd -s dev             # 进入或创建 dev 会话
aicmd -s dev hello       # 用 dev 会话发送请求
aicmd --list-sessions    # 列出会话
aicmd -s dev --empty-session  # 清空并重建 dev 会话
aicmd -m openai:gpt-4o 当前目录有多少文件  # 临时指定模型
```

注意：
- 普通 `aicmd ...` 默认使用每日 session，例如 `cmd-20260617`。
- `-s dev` 如果已存在，会继续写入同一个 session。
- `--empty-session` 会清空会话记录，属于危险操作，会二次确认。

### 7.4 会话查看

AICmd 会把默认每日会话和 `-s` 指定会话保存在 `~/.aicmd/sessions`。可以直接查看历史，不需要调用模型：

```bash
aicmd session                 # 显示当前默认会话名、文件位置和消息数量
aicmd session list            # 列出所有已保存会话
aicmd session show            # 查看当前默认会话最近 20 条消息
aicmd session show dev        # 查看 dev 会话最近 20 条消息
aicmd session show dev --limit 5
aicmd last                    # 查看当前默认会话最后一条非 system 消息
```

这些命令是只读操作，不会清空或修改 session 文件。

### 7.5 脚本工作流：`aicmd do`

当任务不适合一行命令完成时使用，例如处理 CSV、日志、图片或多个文件。

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --plan --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"
aicmd do --output scripts/clean_data.sh "清洗 data/input.csv 并输出 data/output.csv"
aicmd do -f ~/.aicmd/searches/gemini-cli.txt "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --from-search last "根据最近一次搜索记录生成执行脚本"
```

AICmd 会让 LLM 生成“创建脚本并运行脚本”的命令，并走正常确认流程。

先搜索、保存记录、再用 `do` 执行：

```bash
# 方式 1：搜索时直接保存
aicmd search "gemini-cli 官方安装方式" --save gemini-cli

# 方式 2：先搜索；满意后保存上一条搜索结果
aicmd search "gemini-cli 官方安装方式"
aicmd search save gemini-cli

# 查看保存结果，再作为 do 的上下文
aicmd search show gemini-cli
aicmd do --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
```

`--from-search` 会自动读取 `~/.aicmd/searches/<name>.txt`，并把它作为脚本生成上下文。适合“先搜索官方安装方式，再根据搜索记录执行”的场景。如果只存在原始搜索结果 `<name>.raw.txt`，AICmd 会提示先运行 `aicmd search summarize <name>`。
`--plan` 只生成一条安全的计划展示命令，不创建脚本、不安装软件、不修改文件。适合先检查模型是否理解搜索记录和任务风险。

### 7.6 报错诊断：`aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

AICmd 会先执行该命令，捕获 stdout/stderr/exit code，然后让 LLM 生成安全的诊断或修复命令。

### 7.7 搜索

```bash
aicmd search "今天 AI 新闻"
aicmd search "DeepSeek latest model"

# 搜索并直接保存；未指定名称时会自动生成名称
aicmd search "gemini-cli 官方安装方式" --save
aicmd search "gemini-cli 官方安装方式" --save gemini-cli

# 先搜索，满意后保存上一条搜索结果
aicmd search save
aicmd search save gemini-cli

# 如果搜索完成但模型整理失败，稍后重新整理原始搜索结果
aicmd search summarize last
aicmd search summarize gemini-cli

# 查看保存结果
aicmd search list
aicmd search show gemini-cli
aicmd search show last
aicmd search open gemini-cli
aicmd search rm gemini-cli
```

`aicmd search` 会先调用已配置的搜索 MCP server，再把 MCP 结果发送给 LLM，输出适合终端阅读的总结。
每次搜索都会写入最近一次搜索：`~/.aicmd/searches/.last.txt`。使用 `--save` 或 `aicmd search save` 时，会保存为命名记录，例如 `~/.aicmd/searches/gemini-cli.txt`。
如果 MCP 搜索已经完成，但 LLM 总结阶段因为模型过载或接口错误失败，AICmd 会保留原始搜索结果：`~/.aicmd/searches/.last.raw.txt`。如果搜索时使用了 `--save gemini-cli`，也会保留 `~/.aicmd/searches/gemini-cli.raw.txt`。稍后可运行 `aicmd search summarize last` 或 `aicmd search summarize gemini-cli` 重新整理。
`aicmd search list` 会显示每条记录的状态：`summary`、`raw` 或 `summary+raw`。`aicmd search rm <name>` 会同时删除 `<name>.txt` 和 `<name>.raw.txt`。

对普通用户来说，只需要记住 `aicmd search` 这一个搜索入口。

### 7.8 模型与配置命令

```bash
aicmd config init            # 从 .env 生成 ~/.aicmd/config.yaml
aicmd config init --force    # 重新从 .env 生成配置，会二次确认
aicmd config path            # 输出 ~/.aicmd/config.yaml 路径
aicmd config dir             # 输出 ~/.aicmd 目录
aicmd config show            # 输出 config.yaml
aicmd config edit            # 编辑 config.yaml
aicmd config mcp             # 输出 ~/.aicmd/mcp.json 路径
aicmd config doctor          # 等同于 aicmd doctor
aicmd doctor                 # 检查本机 AICmd 运行状态

# 兼容入口
aicmd init --from-env        # 等同于 aicmd model init --from-env
aicmd model path             # 输出 ~/.aicmd/config.yaml 路径
```

### 7.9 完整命令索引

这一节按命令列出用途、常用写法和注意事项。

| 命令 | 用途 | 常用示例 | 注意事项 |
| --- | --- | --- | --- |
| `aicmd <自然语言>` | 生成一条终端命令，确认后执行。 | `aicmd 当前目录有多少文件` | 执行前会出现 `execute/revise/describe/copy/quit`。执行结果会写入当前 session。 |
| `aicmd -m <MODEL> <任务>` | 临时切换模型。 | `aicmd -m openai:gpt-4o 当前目录有多少文件` | 只影响本次请求，不修改 `config.yaml`。 |
| `aicmd -s [SESSION] [任务]` | 查看、进入或创建 session。 | `aicmd -s`, `aicmd -s dev hello` | 不带任务时只准备 session；带任务时会发送请求。 |
| `aicmd --empty-session` | 清空当前或指定 session。 | `aicmd -s dev --empty-session` | 会二次确认；清空后历史上下文不可用。 |
| `aicmd -f <FILE> <任务>` | 给普通请求附加文件、目录或 URL 上下文。 | `aicmd -f README.md 总结这个文件` | 适合一次性让模型读取文件。复杂脚本任务优先用 `aicmd do -f`。 |
| `aicmd --dry-run <任务>` | 预览请求，不执行最终命令。 | `aicmd --dry-run 当前目录有多少文件` | 会显示当前 prompt/session 上下文，适合排查上下文是否过长。 |
| `aicmd do <任务>` | 为复杂任务生成脚本并进入确认执行流程。 | `aicmd do "处理 input.csv，输出 cleaned.csv"` | 默认脚本写入 `.aicmd/task-时间戳.sh` 或 `.ps1`。 |
| `aicmd do --plan <任务>` | 只生成执行计划。 | `aicmd do --plan "安装 Docker"` | 不创建脚本、不安装软件、不修改文件。 |
| `aicmd do --dry-run <任务>` | 预览 `do` 生成的任务 prompt。 | `aicmd do --dry-run "统计日志"` | 用于检查任务描述、参考文件、搜索记录是否已注入。 |
| `aicmd do -f <FILE> <任务>` | 把文本文件作为脚本任务参考。 | `aicmd do -f notes.txt "按说明执行"` | `-f` 目前只支持普通文件。 |
| `aicmd do --from-search <NAME> <任务>` | 使用已保存搜索结果作为脚本上下文。 | `aicmd do --from-search docker-install "安装 Docker"` | 读取 `~/.aicmd/searches/<NAME>.txt`；`last` 表示最近一次搜索。 |
| `aicmd do -o <PATH> <任务>` | 指定生成脚本路径。 | `aicmd do -o scripts/task.sh "清洗 CSV"` | 适合把脚本长期保存到项目目录。 |
| `aicmd search <查询>` | 调用 MCP 搜索并让 LLM 整理结果。 | `aicmd search "今天 AI 新闻"` | 每次会写入 `.last.txt` 和 `.last.raw.txt`。 |
| `aicmd search <查询> --save [NAME]` | 搜索并立即保存命名记录。 | `aicmd search "Docker 安装" --save docker-install` | 生成 `<NAME>.txt` 和 `<NAME>.raw.txt`；不写名称时自动生成。 |
| `aicmd search save [NAME]` | 保存最近一次搜索整理结果。 | `aicmd search save docker-install` | 适合先搜索、满意后再保存。 |
| `aicmd search summarize [NAME\|last]` | 从 raw 搜索结果重新整理。 | `aicmd search summarize docker-install` | 当模型临时失败但 raw 已保存时使用。 |
| `aicmd search list` | 列出搜索记录。 | `aicmd search list` | 状态包含 `summary`、`raw`、`summary+raw`。 |
| `aicmd search show <NAME\|last>` | 输出搜索整理结果。 | `aicmd search show docker-install` | 只读，不调用模型。 |
| `aicmd search open <NAME\|last>` | 打开搜索记录文件。 | `aicmd search open docker-install` | 优先使用 `$EDITOR`，否则使用系统默认打开方式。 |
| `aicmd search rm <NAME>` | 删除搜索记录。 | `aicmd search rm docker-install` | 会删除 `<NAME>.txt` 和 `<NAME>.raw.txt`。 |
| `aicmd err -- <命令>` | 执行命令并把报错交给 LLM 诊断。 | `aicmd err -- pnpm test` | 该命令会真实执行；不要直接传危险命令。 |
| `aicmd session` | 显示当前默认 session 信息。 | `aicmd session` | 只读，不调用模型。 |
| `aicmd session list` | 列出所有 session。 | `aicmd session list` | session 存在 `~/.aicmd/sessions`。 |
| `aicmd session show [SESSION] [--limit N]` | 查看 session 最近消息。 | `aicmd session show dev --limit 5` | 默认显示当前每日 session 最近 20 条。 |
| `aicmd last` | 查看当前默认 session 最后一条非 system 消息。 | `aicmd last` | 适合快速回看上一轮输出。 |
| `aicmd config init [--force]` | 从 `.env` 生成 `config.yaml`。 | `aicmd config init --force` | `--force` 会覆盖配置并二次确认。 |
| `aicmd config path` | 输出 `config.yaml` 路径。 | `aicmd config path` | 通常是 `~/.aicmd/config.yaml`。 |
| `aicmd config dir` | 输出 AICmd 配置目录。 | `aicmd config dir` | 通常是 `~/.aicmd`。 |
| `aicmd config show` | 输出当前配置。 | `aicmd config show` | 可能包含 API key，不要贴到公开场合。 |
| `aicmd config edit` | 编辑当前配置。 | `aicmd config edit` | 使用 `$EDITOR`。 |
| `aicmd config mcp` | 输出 MCP 配置路径。 | `aicmd config mcp` | 通常是 `~/.aicmd/mcp.json`。 |
| `aicmd config doctor` | 运行诊断。 | `aicmd config doctor` | 等价于 `aicmd doctor`。 |
| `aicmd model ...` | 模型配置兼容入口。 | `aicmd model show` | 推荐普通用户优先使用 `aicmd config ...`。 |
| `aicmd init --from-env` | 从 `.env` 初始化配置。 | `aicmd init --from-env` | 等价于 `aicmd model init --from-env`。 |
| `aicmd mcp list` | 列出 MCP 命令。 | `aicmd mcp list` | 读取 `~/.aicmd/mcp.json`。 |
| `aicmd mcp <命令> <输入>` | 调用 MCP 并让 LLM 整理。 | `aicmd mcp search "OpenAI latest news"` | 普通搜索建议直接用 `aicmd search`。 |
| `aicmd mcp-raw <命令> <输入>` | 输出 MCP 原始结果。 | `aicmd mcp-raw search "OpenAI latest news"` | 调试 MCP 时使用，不经过 LLM 整理。 |
| `aicmd doctor` | 检查安装和配置状态。 | `aicmd doctor` | 会检查二进制、版本、模型、MCP、PATH、shell 集成。 |
| `aicmd shell-init [shell]` | 输出 shell 集成代码。 | `eval "$(aicmd shell-init)"` | 用于让 `cd` 类命令影响当前终端目录。 |
| `aicmd update --check` | 检查最新版本。 | `aicmd update --check` | 不安装。 |
| `aicmd update` | 更新到最新 Release。 | `aicmd update` | 会二次确认并覆盖本地二进制。 |
| `aicmd update --version <TAG>` | 安装指定版本。 | `aicmd update --version v0.30.18` | 用于回滚或安装指定 Release。 |
| `aicmd update --dry-run` | 显示更新命令但不执行。 | `aicmd update --dry-run` | 用于排查安装器 URL。 |

## 8. 安全注意事项

- 选择 `execute` 前一定要检查生成的命令。
- 对 `rm`、`mv`、`chmod`、`chown`、数据库迁移、云资源操作等危险命令保持谨慎。
- `.env`、`~/.aicmd/config.yaml`、`~/.aicmd/mcp.json` 可能包含 API key，不要提交到公开仓库。
- MCP server 会运行本地命令，例如 `npx ...`，只配置你信任的 MCP server。
- `aicmd err -- <command>` 为了捕获输出，会真实执行这条命令。

## 9. 更新

推荐使用内置更新命令：

```bash
aicmd update --check
aicmd update
aicmd update --version v0.30.18
aicmd update --dry-run
```

`aicmd update --check` 只检查当前版本和最新 Release，不会安装。`aicmd update` 会先检查是否已有新版本；如果已经是最新版本，会直接提示无需更新。需要安装时会二次确认，因为它会重新下载安装并覆盖本地 AICmd。更新完成后建议运行：

```bash
aicmd doctor
```

也可以手动重新运行安装器更新到最新 Release：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

安装指定版本：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.18
```

Windows PowerShell 指定版本：

```powershell
$env:AICMD_VERSION = "v0.30.18"
irm https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 | iex
Remove-Item Env:AICMD_VERSION
# 或下载 install.ps1 后运行：.\install.ps1 -Version v0.30.18
```

## 10. 常见问题

### `aicmd: command not found`

请确认安装目录已加入 `PATH`，然后新开终端。

macOS / Linux 默认：

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

### 找不到配置

```bash
aicmd doctor
aicmd config path
aicmd init --from-env
```

### 修改 `.env` 后模型没有变化

重新生成 `config.yaml`：

```bash
aicmd init --from-env --force
```

### Windows 命令输出乱码

Windows 传统 `cmd.exe`/部分系统命令可能使用本地代码页，例如中文系统常见的 GBK/CP936，而不是 UTF-8。AICmd v0.30.18 起会在 Windows 执行命令时优先按 UTF-8 解码，失败后回退到 GBK，减少中文输出乱码。

如果仍然乱码，可以在 PowerShell 中先尝试：

```powershell
chcp 65001
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
```

### `timed out waiting for MCP response`

这通常表示 MCP server 已经启动，但在超时时间内没有完成握手、工具列表或工具调用。Windows 上首次运行 `npx -y ...` 时可能需要下载 MCP 包，耗时会更长。AICmd 默认等待 MCP 启动/工具列表 180 秒，工具调用 300 秒。需要时可以临时调大：

```powershell
$env:AICMD_MCP_START_TIMEOUT_SECS = "300"
$env:AICMD_MCP_CALL_TIMEOUT_SECS = "600"
aicmd search "今天北京天气"
```

如果仍然失败，新版会显示具体阶段和 MCP stderr，常见原因包括 Node/npm 未安装、`npx` 不在 PATH、网络无法下载 npm 包、MCP API key 配置错误。

### 搜索命令不可用或 MCP 配置问题

```bash
$EDITOR ~/.aicmd/mcp.json
aicmd search "test"
```

### 执行了 `cd ..` 但当前目录没变

启用 shell 集成，或安装后新开终端：

```bash
eval "$(aicmd shell-init)"
```

## 11. 上游与项目范围

AICmd 复用上游 AIChat 的模型客户端、session、role 和 shell 执行能力，但公开 CLI 会刻意聚焦自然语言终端命令工作流。

更多上游说明：`docs/upstream-aichat.md`
