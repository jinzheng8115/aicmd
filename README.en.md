# AICmd

[中文](README.md)

AICmd turns natural language into safe, reviewable terminal commands. You describe what you want, AICmd generates a shell command, you choose what to do with it, and AICmd can summarize the command output after execution.

Upstream: [sigoden/aichat](https://github.com/sigoden/aichat)
License: MIT OR Apache-2.0

## 1. What AICmd is for

- Generate terminal commands from natural language.
- Run local file/data tasks through a generated script.
- Capture a failing command and ask the LLM for diagnosis/fix commands.
- Call configured MCP tools such as web search, then let the LLM summarize the MCP result.
- Keep the product focused on terminal command workflows, not general chat app features.

## 2. Platform support

Supported release binaries:

| System | CPU / architecture | Release target |
| --- | --- | --- |
| macOS Apple Silicon | arm64 / aarch64 | `aarch64-apple-darwin` |
| macOS Intel | x86_64 | `x86_64-apple-darwin` |
| Linux ARM64 | arm64 / aarch64 | `aarch64-unknown-linux-musl` |
| Linux Intel/AMD | x86_64 | `x86_64-unknown-linux-musl` |
| Windows ARM64 | arm64 / aarch64 | `aarch64-pc-windows-msvc` |
| Windows Intel/AMD | x86_64 | `x86_64-pc-windows-msvc` |

Windows WSL can use the Linux installer.

## 3. Before installation

### 3.1 Required: model `.env`

AICmd needs one LLM model configuration. The simple path is to create a `.env` file before generating `config.yaml`.

Minimal OpenAI-compatible example:

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
```

Supported providers:

| Provider | `AICMD_MODEL_PROVIDER` | Notes |
| --- | --- | --- |
| OpenAI or OpenAI-compatible | `openai` | Requires `AICMD_OPENAI_API_STYLE=chat` or `responses` |
| Anthropic Claude | `anthropic` | Written to config as Claude client |
| Google Gemini | `google` | Written to config as Gemini client |

Common `.env` fields:

```env
# Display/client name. This becomes the client name in config.yaml.
AICMD_MODEL_NAME=openai

# openai | anthropic | google
AICMD_MODEL_PROVIDER=openai

# Provider API base URL.
AICMD_MODEL_API_BASE=https://api.openai.com/v1

# Provider API key.
AICMD_MODEL_API_KEY=sk-xxxx

# One or more provider model ids, comma-separated.
AICMD_MODEL_IDS=gpt-4o,gpt-4.1

# Optional default model. If omitted, AICmd uses MODEL_NAME:first MODEL_ID.
# AICMD_DEFAULT_MODEL=openai:gpt-4o

# OpenAI only: chat | responses
AICMD_OPENAI_API_STYLE=chat
```

If you cloned the repository, you can copy the template:

```bash
cp .env.example .env
$EDITOR .env
```

If you use the one-line binary installer without cloning the repository, create a local `.env` file yourself using the fields above. Later, run `aicmd init --from-env` from the directory containing that `.env`, or set `AICMD_MODEL_ENV=/path/to/.env`.

### 3.2 Optional: MCP `mcp.json`

MCP is configured separately from the LLM model. The runtime MCP file is:

```text
~/.aicmd/mcp.json
```

The installer creates a starter `mcp.json` if the file does not already exist. The recommended setup is to keep your prepared `.env` and `mcp.json` in the same directory, then run `aicmd init --from-env`; AICmd will generate `~/.aicmd/config.yaml` from `.env` and copy `mcp.json` to `~/.aicmd/mcp.json`.

Create or edit it manually:

```bash
mkdir -p ~/.aicmd
$EDITOR ~/.aicmd/mcp.json
```

Example:

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

Notes:
- `servers` defines how to start MCP servers.
- `commands` defines the AICmd command names users type.
- MCP commands return raw tool data first, then AICmd sends it to the configured LLM for a terminal-friendly summary.

## 4. Install

### 4.1 Recommended: binary install, no Rust required

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Windows PowerShell:

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
```

Default install locations:

| Item | macOS / Linux | Windows |
| --- | --- | --- |
| Binary | `~/.local/bin/aicmd` | `~/.local/bin/aicmd.exe` |
| Runtime config | `~/.aicmd/config.yaml` | `~/.aicmd/config.yaml` |
| MCP config | `~/.aicmd/mcp.json` | `~/.aicmd/mcp.json` |

The installer also creates compatibility wrappers for older scripts, such as `aicmd-do`, `aicmd-err`, `aicmd-model`, and `aicmd-shell-init`. New usage should prefer the main `aicmd` command examples below.

### 4.2 Source/developer install, Rust required

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
cp .env.example .env
$EDITOR .env
contrib/aicmd/install.sh --from-source
```

## 5. After installation: generate `config.yaml`

After installing the binary, generate the runtime model config from `.env`. If a `mcp.json` file exists in the same directory, it is copied to `~/.aicmd/mcp.json` at the same time.

```bash
aicmd init --from-env
```

This writes or updates:

```text
~/.aicmd/config.yaml
~/.aicmd/mcp.json   # only when local mcp.json exists
```

AICmd will ask for confirmation before writing. This is intentional because `config.yaml` contains your API key.

If your `.env` is not in the current directory, point AICmd to it. AICmd also looks for `mcp.json` next to that `.env`; you can override the MCP source with `AICMD_MCP_SOURCE=/path/to/mcp.json`.

```bash
AICMD_MODEL_ENV=/path/to/.env aicmd init --from-env
AICMD_MODEL_ENV=/path/to/.env AICMD_MCP_SOURCE=/path/to/mcp.json aicmd init --from-env
```

If `config.yaml` already exists and you want to regenerate it:

```bash
aicmd init --from-env --force
```

Useful checks:

```bash
aicmd model path      # show config.yaml path
aicmd model show      # print config.yaml
aicmd model edit      # edit config.yaml
```

## 6. Shell integration

Shell integration lets commands such as `cd ..` update your current terminal directory after AICmd executes them.

For normal first-time installation, you do not need to run any shell integration command manually.

What the installer does:
- macOS / Linux installer automatically writes shell integration to `~/.zshrc` or `~/.bashrc`.
- Windows installer automatically writes shell integration to your PowerShell profile.
- After installation, open a new terminal or PowerShell window. The integration will be loaded automatically.

Manual enable is only for exceptional cases:
- You installed with `--no-shell-integration` or `-NoProfile`.
- You want the already-open current terminal to load the integration immediately without opening a new terminal.

Manual commands:

```bash
# zsh / bash
eval "$(aicmd shell-init)"
```

```powershell
# PowerShell
Invoke-Expression (& aicmd shell-init powershell)
```

If shell integration is disabled, AICmd can still run commands, but `cd` results cannot update your current terminal directory.

## 7. Command usage

### 7.1 Basic natural-language command

```bash
aicmd 当前目录有多少文件
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
```

AICmd will show a generated command and ask:

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

Choices:
- `execute` / `e`: run the command
- `revise` / `r`: ask the LLM to modify the command
- `describe` / `d`: explain the command in Chinese
- `copy` / `c`: copy the command
- `quit` / `q`: quit without running

After execution, AICmd prints raw command output and asks the LLM to summarize it.

### 7.2 Global options

These options belong to `aicmd` itself. They control how AICmd runs; they are not part of the natural-language task.

| Option | Meaning | Example |
| --- | --- | --- |
| `-m, --model <MODEL>` | Temporarily use a specific model for this request. | `aicmd -m openai:gpt-4o 当前目录有多少文件` |
| `-s, --session [SESSION]` | Show current session when no name is given; otherwise start or join a named session. | `aicmd -s`, `aicmd -s dev hello` |
| `--empty-session` | Clear/recreate the selected session; AICmd asks for confirmation. | `aicmd -s dev --empty-session` |
| `-f, --file <FILE>` | Attach a file, directory, or URL as context for the request. | `aicmd -f README.md summarize this file` |
| `--dry-run` | Show the message/prompt that would be sent, but do not call the LLM. Useful for debugging prompt/session/config behavior. | `aicmd --dry-run 当前目录有多少文件` |
| `--list-sessions` | List saved sessions. | `aicmd --list-sessions` |
| `-h, --help` | Print help. | `aicmd --help` |
| `-V, --version` | Print version. | `aicmd --version` |

There are 8 global options in the current CLI.

Subcommands also have their own options:

| Command | Option | Meaning |
| --- | --- | --- |
| `aicmd do` | `--dry-run` | Build the script-generation request but do not send it to the LLM. |
| `aicmd do` | `-f, --file <FILE>` | Include a saved text file, such as a previous search result, as task context. |
| `aicmd do` | `-o, --output <PATH>` | Ask AICmd to create the task script at this path. |
| `aicmd model init` / `aicmd init` | `--from-env` | Require `.env` and generate `~/.aicmd/config.yaml` from it. |
| `aicmd model init` / `aicmd init` | `--force` | Overwrite existing `config.yaml`; AICmd asks for confirmation. |
| `aicmd shell-init` | `zsh`, `bash`, `powershell` | Print integration code for that shell. Usually not needed after normal install. |

### 7.3 Sessions

```bash
aicmd -s                 # show current/default session
aicmd -s dev             # start or join session dev
aicmd -s dev hello       # use session dev and send a request
aicmd --list-sessions    # list sessions
aicmd -s dev --empty-session  # clear/recreate an empty dev session
aicmd -m openai:gpt-4o 当前目录有多少文件  # temporary model override
```

Notes:
- Plain `aicmd ...` uses the daily default session, such as `cmd-20260617`.
- `-s dev` reuses the same session if it already exists.
- `--empty-session` is destructive and asks for confirmation.

### 7.4 Script workflow: `aicmd do`

Use this when the task is more than a one-liner, for example processing CSV, logs, images, or multiple files.

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"
aicmd do --output scripts/clean_data.sh "清洗 data/input.csv 并输出 data/output.csv"
aicmd do -f .aicmd/notes/gemini-cli-install.txt "根据这份搜索记录，在本机安装 gemini-cli"
```

AICmd asks the LLM to generate commands that create a script, review it, and execute it through the normal confirmation flow.

Search first, save notes, then execute with `do`:

```bash
mkdir -p .aicmd/notes
aicmd search "gemini-cli 官方安装方式" | tee .aicmd/notes/gemini-cli-install.txt
aicmd do -f .aicmd/notes/gemini-cli-install.txt "根据这份搜索记录，在本机安装 gemini-cli"
```

`-f` reads the saved text file and includes it in the script-generation context. This is useful when the installation or operation should follow a previously searched official guide.

### 7.5 Error diagnosis: `aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

AICmd runs the command, captures stdout/stderr/exit code, and asks the LLM to generate safe diagnostic or fix commands.

### 7.6 Search

```bash
aicmd search "今天 AI 新闻"
aicmd search "DeepSeek latest model"
```

`aicmd search` calls the configured search MCP server first, then sends the MCP result to the LLM for final terminal-friendly summary.

For normal users, `aicmd search` is the only search command to remember.

### 7.7 Model/config commands

```bash
aicmd init --from-env        # same as aicmd model init --from-env
aicmd model path             # print ~/.aicmd/config.yaml path
aicmd model dir              # print ~/.aicmd directory
aicmd model show             # print config.yaml
aicmd model edit             # edit config.yaml
aicmd model init --force     # overwrite with starter or .env-based config
```

## 8. Safety notes

- Always review generated commands before choosing `execute`.
- Be careful with destructive commands such as `rm`, `mv`, `chmod`, `chown`, database migration, and cloud operations.
- `.env`, `~/.aicmd/config.yaml`, and `~/.aicmd/mcp.json` may contain API keys. Do not commit them to public repositories.
- MCP servers run local commands such as `npx ...`; only configure MCP servers you trust.
- `aicmd err -- <command>` really runs the command to capture output.

## 9. Update

Re-run the installer to update to the latest Release:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

For a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.0
```

Windows PowerShell specific version:

```powershell
iwr https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 -UseBasicParsing | iex
# or download install.ps1 and run: .\install.ps1 -Version v0.30.0
```

## 10. Troubleshooting

### `aicmd: command not found`

Make sure the install directory is in `PATH`, then open a new terminal.

macOS / Linux default:

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

### Config not found

```bash
aicmd model path
aicmd init --from-env
```

### Changed `.env`, but model did not change

Regenerate `config.yaml`:

```bash
aicmd init --from-env --force
```

### Search command not found or MCP config issue

```bash
$EDITOR ~/.aicmd/mcp.json
aicmd search "test"
```

### `cd ..` executed but current directory did not change

Enable shell integration or open a new terminal after installation:

```bash
eval "$(aicmd shell-init)"
```

## 11. Upstream scope

AICmd reuses upstream AIChat internals for model clients, sessions, roles, and shell execution, but the public CLI is intentionally focused on natural-language terminal command workflows.

More upstream attribution: `docs/upstream-aichat.md`
