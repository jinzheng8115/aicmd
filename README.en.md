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
irm https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 | iex
```

Note: do not use `iwr ... | iex`. `iwr` / `Invoke-WebRequest` returns a response object, not script text. If you must use `iwr`, use `(iwr URL -UseBasicParsing).Content | iex`.

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
Invoke-Expression ((& aicmd shell-init powershell) -join [Environment]::NewLine)
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

After execution, AICmd prints raw command output and asks the LLM to summarize it. The command, exit code, truncated stdout/stderr, and summary are stored in the current session so the next turn can refer to the previous execution result.

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
| `aicmd do` | `--plan` | Generate an execution plan only; do not create or run a task script. |
| `aicmd do` | `-f, --file <FILE>` | Include a saved text file, such as a previous search result, as task context. |
| `aicmd do` | `--from-search <NAME>` | Include a saved search result such as `~/.aicmd/searches/<NAME>.txt`. |
| `aicmd do` | `-o, --output <PATH>` | Ask AICmd to create the task script at this path. |
| `aicmd model init` / `aicmd init` | `--from-env` | Require `.env` and generate `~/.aicmd/config.yaml` from it. |
| `aicmd model init` / `aicmd init` | `--force` | Overwrite existing `config.yaml`; AICmd asks for confirmation. |
| `aicmd shell-init` | `zsh`, `bash`, `powershell` | Print integration code for that shell. Usually not needed after normal install. |
| `aicmd doctor` | none | Check install, model config, MCP/search, PATH, and shell integration status. |
| `aicmd session` | `list`, `show`, `--limit` | Inspect current session, saved sessions, and recent messages. |
| `aicmd last` | none | Show the last non-system message in the current default session. |
| `aicmd search` | `summarize <name|last>` | Summarize a saved raw search result again. |
| `aicmd search` | `list`, `show <name|last>` | List or show saved searches; list shows summary/raw status. |
| `aicmd search` | `open <name|last>`, `rm <name>` | Open or remove saved search records. |
| `aicmd update` | `--check`, `--version`, `--dry-run` | Check or update AICmd with the official installer. |

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

### 7.4 Inspect sessions

AICmd stores the default daily session and `-s` sessions under `~/.aicmd/sessions`. You can inspect history without calling the model:

```bash
aicmd session                 # show current default session name, file path, and message count
aicmd session list            # list saved sessions
aicmd session show            # show the latest 20 messages in the current default session
aicmd session show dev        # show the latest 20 messages in session dev
aicmd session show dev --limit 5
aicmd last                    # show the last non-system message in the current default session
```

These commands are read-only and do not clear or modify session files.

### 7.5 Script workflow: `aicmd do`

Use this when the task is more than a one-liner, for example processing CSV, logs, images, or multiple files.

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --plan --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"
aicmd do --output scripts/clean_data.sh "清洗 data/input.csv 并输出 data/output.csv"
aicmd do -f ~/.aicmd/searches/gemini-cli.txt "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
aicmd do --from-search last "根据最近一次搜索记录生成执行脚本"
```

AICmd asks the LLM to generate commands that create a script, review it, and execute it through the normal confirmation flow.

Search first, save notes, then execute with `do`:

```bash
# Option 1: search and save immediately
aicmd search "gemini-cli 官方安装方式" --save gemini-cli

# Option 2: search first; save the last result after reviewing it
aicmd search "gemini-cli 官方安装方式"
aicmd search save gemini-cli

# Inspect the saved result, then use it as do context
aicmd search show gemini-cli
aicmd do --from-search gemini-cli "根据这份搜索记录，在本机安装 gemini-cli"
```

`--from-search` reads `~/.aicmd/searches/<name>.txt` automatically and includes it in the script-generation context. This is useful when the installation or operation should follow a previously searched official guide. If only `<name>.raw.txt` exists, AICmd asks you to run `aicmd search summarize <name>` first.
`--plan` only generates a safe command that prints an execution plan. It does not create scripts, install software, or modify files. Use it to check whether the model understands the saved search and task risks.

### 7.6 Error diagnosis: `aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

AICmd runs the command, captures stdout/stderr/exit code, and asks the LLM to generate safe diagnostic or fix commands.

### 7.7 Search

```bash
aicmd search "今天 AI 新闻"
aicmd search "DeepSeek latest model"

# Search and save immediately; omit the name to auto-generate one
aicmd search "gemini-cli 官方安装方式" --save
aicmd search "gemini-cli 官方安装方式" --save gemini-cli

# Search first, then save the last result after reviewing it
aicmd search save
aicmd search save gemini-cli

# If search completed but LLM summarization failed, retry from the raw result later
aicmd search summarize last
aicmd search summarize gemini-cli

# Inspect saved results
aicmd search list
aicmd search show gemini-cli
aicmd search show last
aicmd search open gemini-cli
aicmd search rm gemini-cli
```

`aicmd search` calls the configured search MCP server first, then sends the MCP result to the LLM for final terminal-friendly summary.
Every search also writes the latest result to `~/.aicmd/searches/.last.txt`. `--save` or `aicmd search save` stores a named record such as `~/.aicmd/searches/gemini-cli.txt`.
If MCP search succeeds but LLM summarization fails because the model is overloaded or the API errors, AICmd keeps the raw search result at `~/.aicmd/searches/.last.raw.txt`. When the search used `--save gemini-cli`, it also keeps `~/.aicmd/searches/gemini-cli.raw.txt`. Later, run `aicmd search summarize last` or `aicmd search summarize gemini-cli` to summarize the saved raw result.
`aicmd search list` shows each record status: `summary`, `raw`, or `summary+raw`. `aicmd search rm <name>` removes both `<name>.txt` and `<name>.raw.txt`.

For normal users, `aicmd search` is the only search entry point to remember.

### 7.8 Model/config commands

```bash
aicmd config init            # generate ~/.aicmd/config.yaml from .env
aicmd config init --force    # regenerate config from .env; asks for confirmation
aicmd config path            # print ~/.aicmd/config.yaml path
aicmd config dir             # print ~/.aicmd directory
aicmd config show            # print config.yaml
aicmd config edit            # edit config.yaml
aicmd config mcp             # print ~/.aicmd/mcp.json path
aicmd config doctor          # same as aicmd doctor
aicmd doctor                 # check local AICmd runtime status

# Compatibility entry points
aicmd init --from-env        # same as aicmd model init --from-env
aicmd model path             # print ~/.aicmd/config.yaml path
```

### 7.9 Complete command reference

This section lists each command, what it does, common usage, and important notes.

| Command | Purpose | Example | Notes |
| --- | --- | --- | --- |
| `aicmd <natural language>` | Generate a terminal command and run it after confirmation. | `aicmd 当前目录有多少文件` | AICmd asks `execute/revise/describe/copy/quit` before running. Results are stored in the current session. |
| `aicmd -m <MODEL> <task>` | Temporarily select a model. | `aicmd -m openai:gpt-4o 当前目录有多少文件` | Affects only this request; does not edit `config.yaml`. |
| `aicmd -s [SESSION] [task]` | Show, start, or join a session. | `aicmd -s`, `aicmd -s dev hello` | With no task it prepares the session; with a task it sends the request. |
| `aicmd --empty-session` | Clear the selected session. | `aicmd -s dev --empty-session` | Requires confirmation. Previous context becomes unavailable. |
| `aicmd -f <FILE> <task>` | Attach a file, directory, or URL as context. | `aicmd -f README.md summarize this file` | Useful for one-off file context. For script tasks, prefer `aicmd do -f`. |
| `aicmd --dry-run <task>` | Preview the request without executing the final command. | `aicmd --dry-run 当前目录有多少文件` | Useful for checking prompt, session, and context size. |
| `aicmd do <task>` | Generate a task script and enter the confirmation flow. | `aicmd do "处理 input.csv，输出 cleaned.csv"` | Default script path is `.aicmd/task-timestamp.sh` or `.ps1`. |
| `aicmd do --plan <task>` | Generate an execution plan only. | `aicmd do --plan "安装 Docker"` | Does not create scripts, install software, or modify files. |
| `aicmd do --dry-run <task>` | Preview the `do` task prompt. | `aicmd do --dry-run "统计日志"` | Checks whether task text, files, and search records are injected. |
| `aicmd do -f <FILE> <task>` | Use a text file as task reference. | `aicmd do -f notes.txt "按说明执行"` | `-f` currently supports regular files. |
| `aicmd do --from-search <NAME> <task>` | Use a saved search result as task context. | `aicmd do --from-search docker-install "安装 Docker"` | Reads `~/.aicmd/searches/<NAME>.txt`; `last` means the latest search. |
| `aicmd do -o <PATH> <task>` | Choose the generated script path. | `aicmd do -o scripts/task.sh "清洗 CSV"` | Useful when the script should remain in the project. |
| `aicmd search <query>` | Call MCP search and summarize with the LLM. | `aicmd search "今天 AI 新闻"` | Writes `.last.txt` and `.last.raw.txt`. |
| `aicmd search <query> --save [NAME]` | Search and save immediately. | `aicmd search "Docker install" --save docker-install` | Writes `<NAME>.txt` and `<NAME>.raw.txt`; omitting name auto-generates one. |
| `aicmd search save [NAME]` | Save the latest search summary. | `aicmd search save docker-install` | Useful after reviewing the search result. |
| `aicmd search summarize [NAME\|last]` | Summarize a raw search result again. | `aicmd search summarize docker-install` | Use when the model failed but raw search was saved. |
| `aicmd search list` | List saved search records. | `aicmd search list` | Status is `summary`, `raw`, or `summary+raw`. |
| `aicmd search show <NAME\|last>` | Print a saved search summary. | `aicmd search show docker-install` | Read-only; does not call the model. |
| `aicmd search open <NAME\|last>` | Open a saved search file. | `aicmd search open docker-install` | Uses `$EDITOR` first, then the OS opener. |
| `aicmd search rm <NAME>` | Remove saved search files. | `aicmd search rm docker-install` | Removes `<NAME>.txt` and `<NAME>.raw.txt`. |
| `aicmd err -- <command>` | Run a command and ask the LLM to diagnose errors. | `aicmd err -- pnpm test` | Really executes the command; do not pass destructive commands casually. |
| `aicmd session` | Show the current default session. | `aicmd session` | Read-only; does not call the model. |
| `aicmd session list` | List sessions. | `aicmd session list` | Sessions live under `~/.aicmd/sessions`. |
| `aicmd session show [SESSION] [--limit N]` | Show recent session messages. | `aicmd session show dev --limit 5` | Defaults to the current daily session and 20 messages. |
| `aicmd last` | Show the last non-system message. | `aicmd last` | Handy for quickly checking the previous output. |
| `aicmd config init [--force]` | Generate `config.yaml` from `.env`. | `aicmd config init --force` | `--force` overwrites config after confirmation. |
| `aicmd config path` | Print the `config.yaml` path. | `aicmd config path` | Usually `~/.aicmd/config.yaml`. |
| `aicmd config dir` | Print the AICmd config directory. | `aicmd config dir` | Usually `~/.aicmd`. |
| `aicmd config show` | Print current config. | `aicmd config show` | May contain API keys; do not paste publicly. |
| `aicmd config edit` | Edit current config. | `aicmd config edit` | Uses `$EDITOR`. |
| `aicmd config mcp` | Print MCP config path. | `aicmd config mcp` | Usually `~/.aicmd/mcp.json`. |
| `aicmd config doctor` | Run diagnostics. | `aicmd config doctor` | Same as `aicmd doctor`. |
| `aicmd model ...` | Compatibility model config entry point. | `aicmd model show` | Regular users should prefer `aicmd config ...`. |
| `aicmd init --from-env` | Initialize config from `.env`. | `aicmd init --from-env` | Same as `aicmd model init --from-env`. |
| `aicmd mcp list` | List MCP commands. | `aicmd mcp list` | Reads `~/.aicmd/mcp.json`. |
| `aicmd mcp <command> <input>` | Call MCP and summarize with the LLM. | `aicmd mcp search "OpenAI latest news"` | For web search, prefer `aicmd search`. |
| `aicmd mcp-raw <command> <input>` | Print raw MCP output. | `aicmd mcp-raw search "OpenAI latest news"` | For debugging MCP; no LLM summary. |
| `aicmd doctor` | Check local install and config status. | `aicmd doctor` | Checks binary, version, model, MCP, PATH, and shell integration. |
| `aicmd shell-init [shell]` | Print shell integration code. | `eval "$(aicmd shell-init)"` | Lets `cd` commands update the current terminal directory. |
| `aicmd update --check` | Check latest version. | `aicmd update --check` | Does not install. |
| `aicmd update` | Update to the latest Release. | `aicmd update` | Confirms before downloading and overwriting the binary. |
| `aicmd update --version <TAG>` | Install a specific version. | `aicmd update --version v0.30.18` | Useful for rollback or pinning. |
| `aicmd update --dry-run` | Print the update command only. | `aicmd update --dry-run` | Useful for checking the installer URL. |

## 8. Safety notes

- Always review generated commands before choosing `execute`.
- Be careful with destructive commands such as `rm`, `mv`, `chmod`, `chown`, database migration, and cloud operations.
- `.env`, `~/.aicmd/config.yaml`, and `~/.aicmd/mcp.json` may contain API keys. Do not commit them to public repositories.
- MCP servers run local commands such as `npx ...`; only configure MCP servers you trust.
- `aicmd err -- <command>` really runs the command to capture output.

## 9. Update

Recommended:

```bash
aicmd update --check
aicmd update
aicmd update --version v0.30.18
aicmd update --dry-run
```

`aicmd update --check` only checks the current version and latest Release; it does not install anything. `aicmd update` checks whether a newer Release exists first; if AICmd is already up to date, it exits without reinstalling. When installation is needed, it asks for confirmation because it downloads and overwrites the local AICmd binary. After updating, run:

```bash
aicmd doctor
```

You can also re-run the installer manually to update to the latest Release:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

For a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash -s -- --version v0.30.18
```

Windows PowerShell specific version:

```powershell
$env:AICMD_VERSION = "v0.30.18"
irm https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1 | iex
Remove-Item Env:AICMD_VERSION
# or download install.ps1 and run: .\install.ps1 -Version v0.30.18
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
aicmd doctor
aicmd config path
aicmd init --from-env
```

### Changed `.env`, but model did not change

Regenerate `config.yaml`:

```bash
aicmd init --from-env --force
```

### Garbled command output on Windows

Traditional Windows `cmd.exe` and some system commands may use the local code page, such as GBK/CP936 on Chinese systems, instead of UTF-8. Starting with AICmd v0.30.18, Windows command output is decoded as UTF-8 first and then falls back to GBK to reduce garbled Chinese output.

If output is still garbled, try this in PowerShell first:

```powershell
chcp 65001
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
```

### `timed out waiting for MCP response`

This means the MCP server process started but did not complete initialization, tool discovery, or tool execution before the timeout. On Windows, the first `npx -y ...` run may take longer because npm needs to download the MCP package. AICmd waits 180 seconds for MCP startup/tool listing and 300 seconds for tool calls by default. You can temporarily increase the timeouts:

```powershell
$env:AICMD_MCP_START_TIMEOUT_SECS = "300"
$env:AICMD_MCP_CALL_TIMEOUT_SECS = "600"
aicmd search "weather in Beijing today"
```

If it still fails, newer AICmd versions print the exact MCP phase and MCP stderr. Common causes include missing Node/npm, `npx` not in PATH, npm download/network issues, or an invalid MCP API key.

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
