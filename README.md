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
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
```

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
Invoke-Expression (& aicmd shell-init powershell)
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

执行后，AICmd 会先输出原始命令结果，再让 LLM 做 summary。

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
| `aicmd do` | `-f, --file <FILE>` | 把保存的文本文件作为任务上下文，例如之前的搜索结果。 |
| `aicmd do` | `-o, --output <PATH>` | 指定生成脚本路径。 |
| `aicmd model init` / `aicmd init` | `--from-env` | 必须读取 `.env` 并生成 `~/.aicmd/config.yaml`。 |
| `aicmd model init` / `aicmd init` | `--force` | 覆盖已有 `config.yaml`，会二次确认。 |
| `aicmd shell-init` | `zsh`, `bash`, `powershell` | 输出对应 shell 的集成代码；正常安装后通常不需要手动执行。 |

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

### 7.4 脚本工作流：`aicmd do`

当任务不适合一行命令完成时使用，例如处理 CSV、日志、图片或多个文件。

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"
aicmd do --output scripts/clean_data.sh "清洗 data/input.csv 并输出 data/output.csv"
aicmd do -f .aicmd/notes/gemini-cli-install.txt "根据这份搜索记录，在本机安装 gemini-cli"
```

AICmd 会让 LLM 生成“创建脚本并运行脚本”的命令，并走正常确认流程。

先搜索、保存记录、再用 `do` 执行：

```bash
mkdir -p .aicmd/notes
aicmd search "gemini-cli 官方安装方式" | tee .aicmd/notes/gemini-cli-install.txt
aicmd do -f .aicmd/notes/gemini-cli-install.txt "根据这份搜索记录，在本机安装 gemini-cli"
```

`-f` 会读取保存的文本文件，并把它作为脚本生成上下文。适合“先搜索官方安装方式，再根据搜索记录执行”的场景。

### 7.5 报错诊断：`aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

AICmd 会先执行该命令，捕获 stdout/stderr/exit code，然后让 LLM 生成安全的诊断或修复命令。

### 7.6 搜索

```bash
aicmd search "今天 AI 新闻"
aicmd search "DeepSeek latest model"
```

`aicmd search` 会先调用已配置的搜索 MCP server，再把 MCP 结果发送给 LLM，输出适合终端阅读的总结。

对普通用户来说，只需要记住 `aicmd search` 这一个搜索命令。

### 7.7 模型与配置命令

```bash
aicmd init --from-env        # 等同于 aicmd model init --from-env
aicmd model path             # 输出 ~/.aicmd/config.yaml 路径
aicmd model dir              # 输出 ~/.aicmd 目录
aicmd model show             # 输出 config.yaml
aicmd model edit             # 编辑 config.yaml
aicmd model init --force     # 覆盖生成 starter 或基于 .env 的配置
```

## 8. 安全注意事项

- 选择 `execute` 前一定要检查生成的命令。
- 对 `rm`、`mv`、`chmod`、`chown`、数据库迁移、云资源操作等危险命令保持谨慎。
- `.env`、`~/.aicmd/config.yaml`、`~/.aicmd/mcp.json` 可能包含 API key，不要提交到公开仓库。
- MCP server 会运行本地命令，例如 `npx ...`，只配置你信任的 MCP server。
- `aicmd err -- <command>` 为了捕获输出，会真实执行这条命令。

## 9. 更新

重新运行安装器即可更新到最新 Release：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

安装指定版本：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.0
```

Windows PowerShell 指定版本：

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
# 或下载 install.ps1 后运行：.\install.ps1 -Version v0.30.0
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
aicmd model path
aicmd init --from-env
```

### 修改 `.env` 后模型没有变化

重新生成 `config.yaml`：

```bash
aicmd init --from-env --force
```

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
