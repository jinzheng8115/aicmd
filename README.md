# AICmd

AICmd turns natural language into safe, reviewable terminal commands. You describe what you want, AICmd generates a shell command, you choose what to do with it, and AICmd can summarize the command output after execution.

AICmd 用自然语言生成可检查、可确认的终端命令。你描述要做什么，AICmd 生成 shell 命令；你确认后执行，执行完成后还可以由 LLM 总结命令输出。

Upstream / 上游来源：[sigoden/aichat](https://github.com/sigoden/aichat)
License / 许可证：MIT OR Apache-2.0

## 1. What AICmd is for / AICmd 适合做什么

English:
- Generate terminal commands from natural language.
- Run local file/data tasks through a generated script.
- Capture a failing command and ask the LLM for diagnosis/fix commands.
- Call configured MCP tools such as web search, then let the LLM summarize the MCP result.
- Keep the product focused on terminal command workflows, not general chat app features.

中文：
- 用自然语言生成终端命令。
- 为本地文件/数据处理任务生成脚本并执行。
- 捕获报错命令的输出，让 LLM 生成诊断/修复命令。
- 调用已配置的 MCP 工具，例如 web search，再让 LLM 整理结果。
- 项目聚焦“自然语言运行终端命令”，不追求通用聊天应用的全部功能。

## 2. Platform support / 平台支持

Supported release binaries / 已发布的二进制平台：

| System / 系统 | CPU / 架构 | Release target |
| --- | --- | --- |
| macOS Apple Silicon | arm64 / aarch64 | `aarch64-apple-darwin` |
| macOS Intel | x86_64 | `x86_64-apple-darwin` |
| Linux ARM64 | arm64 / aarch64 | `aarch64-unknown-linux-musl` |
| Linux Intel/AMD | x86_64 | `x86_64-unknown-linux-musl` |
| Windows ARM64 | arm64 / aarch64 | `aarch64-pc-windows-msvc` |
| Windows Intel/AMD | x86_64 | `x86_64-pc-windows-msvc` |

Windows WSL can use the Linux installer.

Windows WSL 可以使用 Linux 安装方式。

## 3. Before installation / 安装前准备

### 3.1 Required: model `.env` / 必填：模型 `.env`

AICmd needs one LLM model configuration. The simple path is to create a `.env` file before generating `config.yaml`.

AICmd 需要一组 LLM 模型配置。推荐先准备 `.env`，再由它生成 `config.yaml`。

Minimal OpenAI-compatible example / 最小 OpenAI 兼容示例：

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
```

Supported providers / 支持的模型接口：

| Provider / 接口 | `AICMD_MODEL_PROVIDER` | Notes / 说明 |
| --- | --- | --- |
| OpenAI or OpenAI-compatible / OpenAI 或兼容接口 | `openai` | Requires `AICMD_OPENAI_API_STYLE=chat` or `responses` |
| Anthropic Claude | `anthropic` | Written to config as Claude client |
| Google Gemini | `google` | Written to config as Gemini client |

Common `.env` fields / 常用 `.env` 字段：

```env
# Display/client name. This becomes the client name in config.yaml.
# 显示名/客户端名，会写入 config.yaml。
AICMD_MODEL_NAME=openai

# openai | anthropic | google
AICMD_MODEL_PROVIDER=openai

# Provider API base URL.
# 模型 API 地址。
AICMD_MODEL_API_BASE=https://api.openai.com/v1

# Provider API key.
# 模型 API Key。
AICMD_MODEL_API_KEY=sk-xxxx

# One or more provider model ids, comma-separated.
# 一个或多个模型 ID，用英文逗号分隔。
AICMD_MODEL_IDS=gpt-4o,gpt-4.1

# Optional default model. If omitted, AICmd uses MODEL_NAME:first MODEL_ID.
# 可选默认模型。不填时使用 MODEL_NAME:第一个 MODEL_ID。
# AICMD_DEFAULT_MODEL=openai:gpt-4o

# OpenAI only: chat | responses
# 仅 OpenAI 或兼容接口需要：chat | responses
AICMD_OPENAI_API_STYLE=chat
```

You can also copy the repository template:

也可以复制仓库模板：

```bash
cp .env.example .env
$EDITOR .env
```

If you use the one-line binary installer without cloning the repository, create a local `.env` file yourself using the fields above. Later, run `aicmd init --from-env` from the directory containing that `.env`, or set `AICMD_MODEL_ENV=/path/to/.env`.

如果你使用一行命令二进制安装、没有 clone 仓库，请根据上面的字段自己创建一个本地 `.env` 文件。后续在这个 `.env` 所在目录运行 `aicmd init --from-env`，或设置 `AICMD_MODEL_ENV=/path/to/.env`。

### 3.2 Optional: MCP `mcp.json` / 可选：MCP `mcp.json`

MCP is configured separately from the LLM model. The runtime MCP file is:

MCP 与模型配置分开。运行时 MCP 文件是：

```text
~/.aicmd/mcp.json
```

The installer creates a starter `mcp.json` if the file does not already exist. If you want to prepare it before installation, create `~/.aicmd/mcp.json` first; the installer will keep the existing file.

如果该文件不存在，安装器会创建一个 starter `mcp.json`。如果你想在安装前配置 MCP，可以先创建 `~/.aicmd/mcp.json`；安装器会保留已有文件。

Create or edit it manually / 手动创建或编辑：

```bash
mkdir -p ~/.aicmd
$EDITOR ~/.aicmd/mcp.json
```

Example / 示例：

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

Notes / 注意：
- `servers` defines how to start MCP servers.
- `commands` defines the AICmd command names users type.
- `tool` is optional. If omitted, AICmd calls `tools/list` and auto-selects a matching MCP tool.
- MCP commands return raw tool data first, then AICmd sends it to the configured LLM for a terminal-friendly summary.

中文说明：
- `servers` 定义如何启动 MCP server。
- `commands` 定义用户在 AICmd 里输入的命令名。
- `tool` 可以不写。未配置时，AICmd 会调用 `tools/list` 并自动选择匹配的 MCP tool。
- MCP 命令会先拿到工具原始结果，再交给当前 LLM 整理成适合终端阅读的输出。

## 4. Install / 安装

### 4.1 Recommended: binary install, no Rust required / 推荐：二进制安装，不需要 Rust

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Windows PowerShell:

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
```

Default install locations / 默认安装位置：

| Item / 内容 | macOS / Linux | Windows |
| --- | --- | --- |
| Binary / 主程序 | `~/.local/bin/aicmd` | `~/.local/bin/aicmd.exe` |
| Runtime config / 运行时配置 | `~/.aicmd/config.yaml` | `~/.aicmd/config.yaml` |
| MCP config / MCP 配置 | `~/.aicmd/mcp.json` | `~/.aicmd/mcp.json` |

The installer also creates compatibility wrappers:

安装器还会创建兼容 wrapper：

```text
aicmd-do          -> aicmd do
aicmd-err         -> aicmd err
aicmd-model       -> aicmd model
aicmd-mcp         -> aicmd mcp-raw
aicmd-shell-init  -> aicmd shell-init
```

### 4.2 Source/developer install, Rust required / 源码或开发安装，需要 Rust

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
cp .env.example .env
$EDITOR .env
contrib/aicmd/install.sh --from-source
```

## 5. After installation: generate `config.yaml` / 安装后：生成 `config.yaml`

After installing the binary, generate the runtime model config from `.env`:

安装二进制后，用 `.env` 生成运行时模型配置：

```bash
aicmd init --from-env
```

This writes:

这会写入：

```text
~/.aicmd/config.yaml
```

AICmd will ask for confirmation before writing. This is intentional because `config.yaml` contains your API key.

AICmd 写入前会二次确认。这是有意设计，因为 `config.yaml` 会包含你的 API key。

If your `.env` is not in the current directory, point AICmd to it:

如果 `.env` 不在当前目录，可以显式指定：

```bash
AICMD_MODEL_ENV=/path/to/.env aicmd init --from-env
```

If `config.yaml` already exists and you want to regenerate it:

如果 `config.yaml` 已存在并且你要重新生成：

```bash
aicmd init --from-env --force
```

Useful checks / 常用检查：

```bash
aicmd model path      # show config.yaml path / 查看 config.yaml 路径
aicmd model show      # print config.yaml / 输出 config.yaml
aicmd model edit      # edit config.yaml / 编辑 config.yaml
aicmd mcp list        # list MCP commands / 列出 MCP 命令
```

## 6. Shell integration / Shell 集成

AICmd normally runs commands in a child process. A child process cannot change the parent terminal directory, so commands like `cd ..` need shell integration if you want your current terminal to actually move directory.

AICmd 通常在子进程里执行命令。子进程不能改变父终端的当前目录，所以如果你希望 `cd ..` 这类命令真的让当前终端切换目录，需要 shell 集成。

What the installer does / 安装器行为：
- macOS / Linux installer writes shell integration to `~/.zshrc` or `~/.bashrc` unless `--no-shell-integration` is used.
- Windows installer writes PowerShell integration to your PowerShell profile unless `-NoProfile` is used.
- Existing terminals may need to reload the profile or start a new terminal.

中文：
- macOS / Linux 安装器会写入 `~/.zshrc` 或 `~/.bashrc`，除非使用 `--no-shell-integration`。
- Windows 安装器会写入 PowerShell profile，除非使用 `-NoProfile`。
- 已经打开的终端需要重新加载配置或新开终端。

Manual enable / 手动启用：

```bash
# zsh / bash
eval "$(aicmd shell-init)"
```

```powershell
# PowerShell
Invoke-Expression (& aicmd shell-init powershell)
```

If you do not use shell integration, AICmd can still run commands, but `cd` results cannot update your current terminal directory.

如果不启用 shell 集成，AICmd 仍然可以执行命令，但 `cd` 的结果不会同步到当前终端目录。

## 7. Command usage / 命令用法

### 7.1 Basic natural-language command / 基础自然语言命令

```bash
aicmd 当前目录有多少文件
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
```

AICmd will show a generated command and ask:

AICmd 会显示生成的命令并询问：

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

Choices / 选项含义：
- `execute` / `e`: run the command / 执行命令
- `revise` / `r`: ask the LLM to modify the command / 让 LLM 修改命令
- `describe` / `d`: explain the command in Chinese / 用中文解释命令
- `copy` / `c`: copy the command / 复制命令
- `quit` / `q`: quit without running / 退出，不执行

After execution, AICmd prints raw command output and asks the LLM to summarize it.

执行后，AICmd 会先输出原始命令结果，再让 LLM 做 summary。

### 7.2 Sessions / 会话

```bash
aicmd -s                 # show current/default session / 显示当前默认 session
aicmd -s dev             # start or join session dev / 进入或创建 dev 会话
aicmd -s dev hello       # use session dev and send a request / 用 dev 会话发送请求
aicmd --list-sessions    # list sessions / 列出会话
aicmd -s dev --empty-session  # clear/recreate an empty dev session / 清空并重建 dev 会话
aicmd -m openai:gpt-4o 当前目录有多少文件  # temporary model override / 临时指定模型
```

Notes / 注意：
- Plain `aicmd ...` uses the daily default session, such as `cmd-20260617`.
- `-s dev` reuses the same session if it already exists.
- `--empty-session` is destructive and asks for confirmation.

中文：
- 普通 `aicmd ...` 默认使用每日 session，例如 `cmd-20260617`。
- `-s dev` 如果已存在，会继续写入同一个 session。
- `--empty-session` 会清空会话记录，属于危险操作，会二次确认。

### 7.3 Script workflow: `aicmd do` / 脚本工作流

Use this when the task is more than a one-liner, for example processing CSV, logs, images, or multiple files.

当任务不适合一行命令完成时使用，例如处理 CSV、日志、图片或多个文件。

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"
aicmd do --output scripts/clean_data.sh "清洗 data/input.csv 并输出 data/output.csv"
```

AICmd asks the LLM to generate commands that create a script, review it, and execute it through the normal confirmation flow.

AICmd 会让 LLM 生成“创建脚本并运行脚本”的命令，并走正常确认流程。

### 7.4 Error diagnosis: `aicmd err` / 报错诊断

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

AICmd runs the command, captures stdout/stderr/exit code, and asks the LLM to generate safe diagnostic or fix commands.

AICmd 会先执行该命令，捕获 stdout/stderr/exit code，然后让 LLM 生成安全的诊断或修复命令。

### 7.5 MCP and search / MCP 与搜索

```bash
aicmd mcp list
aicmd search "今天 AI 新闻"
aicmd mcp search "DeepSeek latest model"
aicmd mcp context7-library react
```

User-facing MCP commands call MCP first, then send the MCP result to the LLM for final summary.

面向用户的 MCP 命令会先调用 MCP，再把 MCP 结果发送给 LLM 进行最终整理。

Raw MCP output for debugging / 调试 MCP 原始输出：

```bash
aicmd mcp-raw search "今天 AI 新闻"
aicmd mcp-raw context7-library react
```

### 7.6 Model/config commands / 模型与配置命令

```bash
aicmd init --from-env        # same as aicmd model init --from-env
aicmd model path             # print ~/.aicmd/config.yaml path
aicmd model dir              # print ~/.aicmd directory
aicmd model show             # print config.yaml
aicmd model edit             # edit config.yaml
aicmd model init --force     # overwrite with starter or .env-based config
```

中文：

```bash
aicmd init --from-env        # 等同于 aicmd model init --from-env
aicmd model path             # 输出 ~/.aicmd/config.yaml 路径
aicmd model dir              # 输出 ~/.aicmd 目录
aicmd model show             # 输出 config.yaml
aicmd model edit             # 编辑 config.yaml
aicmd model init --force     # 覆盖生成 starter 或基于 .env 的配置
```

## 8. Safety notes / 安全注意事项

English:
- Always review generated commands before choosing `execute`.
- Be careful with destructive commands such as `rm`, `mv`, `chmod`, `chown`, database migration, and cloud operations.
- `.env`, `~/.aicmd/config.yaml`, and `~/.aicmd/mcp.json` may contain API keys. Do not commit them to public repositories.
- MCP servers run local commands such as `npx ...`; only configure MCP servers you trust.
- `aicmd err -- <command>` really runs the command to capture output.

中文：
- 选择 `execute` 前一定要检查生成的命令。
- 对 `rm`、`mv`、`chmod`、`chown`、数据库迁移、云资源操作等危险命令保持谨慎。
- `.env`、`~/.aicmd/config.yaml`、`~/.aicmd/mcp.json` 可能包含 API key，不要提交到公开仓库。
- MCP server 会运行本地命令，例如 `npx ...`，只配置你信任的 MCP server。
- `aicmd err -- <command>` 为了捕获输出，会真实执行这条命令。

## 9. Update / 更新

Re-run the installer to update to the latest Release:

重新运行安装器即可更新到最新 Release：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

For a specific version / 安装指定版本：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.0
```

Windows PowerShell specific version / Windows PowerShell 指定版本：

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
# or download install.ps1 and run: .\install.ps1 -Version v0.30.0
```

## 10. Troubleshooting / 常见问题

### `aicmd: command not found`

Make sure the install directory is in `PATH`, then open a new terminal.

请确认安装目录已加入 `PATH`，然后新开终端。

macOS / Linux default:

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

### Config not found / 找不到配置

```bash
aicmd model path
aicmd init --from-env
```

### Changed `.env`, but model did not change / 修改 `.env` 后模型没有变化

Regenerate `config.yaml`:

重新生成 `config.yaml`：

```bash
aicmd init --from-env --force
```

### MCP command not found / 找不到 MCP 命令

```bash
aicmd mcp list
$EDITOR ~/.aicmd/mcp.json
```

### `cd ..` executed but current directory did not change / 执行了 `cd ..` 但当前目录没变

Enable shell integration or open a new terminal after installation:

启用 shell 集成，或安装后新开终端：

```bash
eval "$(aicmd shell-init)"
```

## 11. Upstream scope / 上游与项目范围

AICmd reuses upstream AIChat internals for model clients, sessions, roles, and shell execution, but the public CLI is intentionally focused on natural-language terminal command workflows.

AICmd 复用上游 AIChat 的模型客户端、session、role 和 shell 执行能力，但公开 CLI 会刻意聚焦自然语言终端命令工作流。

More upstream attribution / 更多上游说明：`docs/upstream-aichat.md`
