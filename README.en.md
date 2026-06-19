# AICmd

[中文](README.md)

AICmd runs terminal commands from natural language. You describe the goal, AICmd generates a reviewable command or script, and you decide whether to execute it. It can also ask the LLM to summarize command output when useful.

Upstream: [sigoden/aichat](https://github.com/sigoden/aichat)
License: MIT OR Apache-2.0

## 1. Start with these 5 commands

```bash
aicmd <what you want>       # generate one command and ask before execution
aicmd do <complex task>     # multi-step scripts, installs, file/data tasks
aicmd search <question>     # call MCP search, then summarize with the LLM
aicmd setup                # first-time setup or reconfiguration
aicmd doctor               # check install, model, MCP, PATH, and shell integration
```

Examples:

```bash
aicmd how many files are in this directory
aicmd list the 10 largest files in this directory
aicmd do "read data/orders.csv, aggregate amount by user, write output/user_totals.csv"
aicmd search "how to install copilot-cli"
```

## 2. Supported platforms

AICmd officially supports macOS, Linux, and Windows WSL. Native Windows PowerShell/cmd is not supported.

| System | Architecture |
| --- | --- |
| macOS | Apple Silicon / Intel |
| Linux / WSL | ARM64 / x86_64 |

## 3. Install

Recommended binary install, no Rust required:

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Default locations:

| Item | Path |
| --- | --- |
| Binary | `~/.local/bin/aicmd` |
| Model config | `~/.aicmd/config.yaml` |
| MCP config | `~/.aicmd/mcp.json` |
| Sessions | `~/.aicmd/sessions` |
| Searches | `~/.aicmd/searches` |

Source install for development:

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
contrib/aicmd/install.sh --from-source
```

## 4. First-time configuration

### 4.1 Prepare `.env`

AICmd needs one model configuration. Minimal OpenAI-compatible example:

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
```

Supported `AICMD_MODEL_PROVIDER` values:

| Provider | Notes |
| --- | --- |
| `openai` | OpenAI or compatible API; use `AICMD_OPENAI_API_STYLE=chat` or `responses` |
| `anthropic` | Anthropic Claude |
| `google` | Google Gemini |

Optional default model:

```env
AICMD_DEFAULT_MODEL=deepseek:deepseek-chat
```

If omitted, AICmd uses:

```text
AICMD_MODEL_NAME:first model id in AICMD_MODEL_IDS
```

### 4.2 Generate config

Recommended:

```bash
aicmd setup
```

Or generate directly from `.env`:

```bash
aicmd init --from-env
```

Overwrite an existing config:

```bash
aicmd init --from-env --force
```

Generated `config.yaml` defaults include:

```yaml
temperature: 0.1
top_p: null
stream: false
ai_summary: true
```

Disable AI summary by default:

```bash
aicmd config summary off
```

Disable it for one command only:

```bash
aicmd --no-summary how many files are in this directory
```

Inspect config:

```bash
aicmd doctor
aicmd config path
aicmd config show
aicmd config edit
```

## 5. MCP and search

MCP config file:

```text
~/.aicmd/mcp.json
```

Minimal example:

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

```bash
aicmd search "today's AI news"
aicmd search "official gemini-cli install guide" --save gemini-cli
aicmd search list
aicmd search show gemini-cli
aicmd search open gemini-cli
aicmd search rm gemini-cli
```

In an interactive terminal, after search completes:

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

- `save`: save the search result.
- `do`: generate an execution script from the search result and current system environment.
- `open`: open the latest search record.

## 6. Command usage

### 6.1 Regular command

```bash
aicmd how many files are in this directory
aicmd --print how many files are in this directory      # print only, do not execute
aicmd --dry-run how many files are in this directory    # preview the prompt
aicmd --no-summary how many files are in this directory # skip AI summary after execution
```

Before execution, AICmd asks:

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

AICmd shows a risk level. Destructive commands require an extra confirmation.

### 6.2 Script workflow: `aicmd do`

Use it for multi-step tasks, file processing, and installation flows:

```bash
aicmd do "process input.csv and write cleaned.csv"
aicmd do --plan "install Docker"                  # plan only
aicmd do --dry-run "count ERROR in logs/*.log"    # preview do prompt
aicmd do -f notes.txt "follow these instructions"
aicmd do --from-search gemini-cli "install gemini-cli"
aicmd do -o scripts/task.sh "clean CSV"
```

`--from-search` reads `~/.aicmd/searches/<name>.txt` and also includes the current system environment, such as OS, architecture, cwd, and whether `brew/node/npm/git/curl` are available. This helps generate a more reliable script.

### 6.3 Error diagnosis: `aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

It runs the command, captures stdout/stderr/exit code, and asks the LLM to generate a diagnostic or fix command.

### 6.4 Sessions

```bash
aicmd -s                     # show current/default session
aicmd -s dev                 # start or join dev session
aicmd -s dev hello           # send request in dev session
aicmd --list-sessions        # list sessions
aicmd -s dev --empty-session # clear dev session, asks for confirmation
```

Plain `aicmd ...` uses a daily session such as `cmd-20260619`.

Inspect history:

```bash
aicmd session
aicmd session list
aicmd session show
aicmd session show dev --limit 5
aicmd last
```

### 6.5 Config commands

```bash
aicmd config init            # generate config.yaml from .env
aicmd config init --force    # overwrite config, asks for confirmation
aicmd config path            # print config.yaml path
aicmd config dir             # print ~/.aicmd directory
aicmd config show            # print config.yaml; may contain API keys
aicmd config edit            # edit config.yaml with $EDITOR
aicmd config summary status  # show AI summary default
aicmd config summary off     # disable AI summary by default
aicmd config summary on      # enable AI summary by default
aicmd config mcp             # print mcp.json path
aicmd config doctor          # same as aicmd doctor
```

### 6.6 Update

```bash
aicmd update --check
aicmd update
aicmd update --version v0.4.0
aicmd update --dry-run
```

## 7. Shell integration

Shell integration lets commands like `cd ..` affect the current terminal. Normal installs usually configure it automatically. If needed:

```bash
eval "$(aicmd shell-init)"
```

Without shell integration, AICmd can still run commands, but directory changes do not persist in your current shell.

## 8. Safety notes

- Always review generated commands before execution.
- Be careful with `rm`, `mv`, `chmod`, database migrations, and cloud operations.
- `.env`, `~/.aicmd/config.yaml`, and `~/.aicmd/mcp.json` may contain API keys. Do not commit them.
- MCP servers run local commands such as `npx ...`; only configure MCP servers you trust.
- `aicmd err -- <command>` really executes the command you pass in.

## 9. Troubleshooting

### `aicmd: command not found`

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

Then open a new terminal or retry the command.

### Missing config

```bash
aicmd doctor
aicmd config path
aicmd init --from-env
```

### Model did not change after editing `.env`

Regenerate config:

```bash
aicmd init --from-env --force
```

### MCP search timeout

The first `npx -y ...` run may need to download MCP packages. Increase timeout temporarily:

```bash
AICMD_MCP_START_TIMEOUT_SECS=300 AICMD_MCP_CALL_TIMEOUT_SECS=600 aicmd search "weather in Beijing today"
```

You can also check:

```bash
aicmd config mcp
aicmd mcp list
```

### `cd ..` runs but the current directory does not change

Enable shell integration or open a new terminal:

```bash
eval "$(aicmd shell-init)"
```

## 10. Scope

AICmd reuses upstream AIChat internals for model clients, sessions, roles, and shell execution, but the public CLI focuses on natural-language terminal command workflows.

More upstream notes: `docs/upstream-aichat.md`
