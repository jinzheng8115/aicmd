# AICmd Usage Guide / AICmd 使用文档

This document is the detailed command reference for AICmd. The README is the short start guide; this file keeps the longer explanations.

本文档是 AICmd 的详细命令参考。README 是快速开始指南；更完整的说明放在这里。

## 1. Product scope / 产品范围

AICmd is a natural-language terminal command runner. You describe a terminal task, AICmd generates a reviewable command or script, and you decide whether to execute it.

AICmd 是自然语言终端命令工具。你描述终端任务，AICmd 生成可检查的命令或脚本，然后由你决定是否执行。

AICmd is intentionally focused. It does not expose broad upstream AIChat workflows such as Chat REPL, RAG, agents, macros, public role switching, custom tools/functions, or server mode.

AICmd 刻意保持聚焦。它不暴露上游 AIChat 的宽功能，例如 Chat REPL、RAG、agents、macros、公开 role 切换、自定义 tools/functions 或 server mode。

## 2. Supported platforms / 支持平台

AICmd officially supports macOS and Linux. Windows users must run it inside WSL as a Linux application. Native Windows PowerShell/cmd and `.exe` installation are not supported.

AICmd 正式支持 macOS 和 Linux。Windows 用户必须在 WSL 中按 Linux 应用运行。不支持 Windows 原生 PowerShell/cmd 和 `.exe` 安装。

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
language: zh
temperature: 0
top_p: null
stream: false
ai_summary: false
```

AI summary is not automatic by default. After execution, the user can choose whether to generate it.
/ AI summary 默认不自动执行。命令完成后，用户可以选择是否生成。

## 5. Default interactive path / 默认交互入口

Run AICmd without arguments in an interactive terminal:

在交互式终端中不带参数运行 AICmd：

```bash
aicmd
```

The `AICmd>` prompt accepts multiple tasks. It resolves the Beijing-date daily session once, passes that named session to every child task, and returns to the prompt even when a child exits non-zero. Enter `exit`, `quit`, or `.exit`; send EOF; or press `Ctrl-C` to leave.

`AICmd>` 可连续接收多个任务。启动时只解析一次按北京时间命名的每日会话，每个子任务都使用该命名会话；即使子任务以非 0 状态退出，也会回到提示符。输入 `exit`、`quit`、`.exit`，发送 EOF，或按 `Ctrl-C` 即可离开。

For a one-shot task, use `aicmd <task>`. AICmd routes it to command, script, search, or diagnosis. Explicit `do`, `search`, and `err` commands remain advanced controls.

单次任务使用 `aicmd <任务>`。AICmd 会自动路由到命令、脚本、搜索或错误诊断；显式的 `do`、`search` 和 `err` 保留为高级控制入口。

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

Terminal help follows the configured `language` and shows one language at a time.

终端帮助遵循已配置的 `language`，一次只显示一种语言。

## 7. Regular command workflow / 普通命令工作流

Example:

示例：

```bash
aicmd 当前目录有多少文件
```

Before confirmation, AICmd runs the read-only checks declared in the structured plan. If any required check fails, it shows every failure and suggestion, records the result in the active session, and does not execute the command.

确认执行前，AICmd 会运行结构化计划中声明的只读检查。任一必要检查失败时，系统会显示全部失败原因和建议、记录到当前 session，并且不会执行命令。

Supported checks / 支持的检查：

| Type | Purpose / 用途 |
| --- | --- |
| `command_exists` | Executable exists in `PATH`. / 可执行命令存在于 `PATH`。 |
| `path_exists` | File or directory exists. / 文件或目录存在。 |
| `path_writable` | Target or nearest existing parent is writable. / 目标或最近存在的父目录可写。 |
| `env_exists` | Environment-variable name exists; its value is never displayed or saved. / 环境变量名存在；变量值不会显示或保存。 |
| `os` | Current OS matches `macos` or `linux`. / 当前系统符合 `macos` 或 `linux`。 |
| `git_clean` | Git working tree has no uncommitted changes. / Git 工作区没有未提交改动。 |

Example plan / 计划示例：

```json
{
  "mode": "direct",
  "command": "python3 task.py",
  "query": "",
  "problem": "",
  "preflight": [
    {
      "type": "command_exists",
      "value": "python3",
      "failure_message": "未找到 Python 3",
      "suggestion": "请先安装 Python 3"
    }
  ]
}
```

Checks are read-only. They do not install dependencies, repair the environment, probe sudo passwords, or elevate privileges. `--dry-run` shows the full planner prompt containing the check contract, while `--print` prints only the generated command. Neither executes checks.

检查是只读的，不会安装依赖、修复环境、测试 sudo 密码或提升权限。`--dry-run` 显示包含检查协议的完整规划 prompt，`--print` 只输出生成的命令，两者都不会执行检查。

Before execution, AICmd shows:

执行前，AICmd 会显示：

```text
Run? [Y/n/?] / 执行？[Y/n/?]
```

Meaning:

含义：

```text
Enter/y    run the generated command / 执行生成命令
n          exit without execution / 不执行并退出
?          show revise, explain, and copy actions / 显示修改、解释、复制等高级选项
```

Useful flags:

常用参数：

```bash
aicmd --print 当前目录有多少文件       # print command only / 只打印命令
aicmd --dry-run 当前目录有多少文件     # preview prompt / 预览 prompt
aicmd --no-summary 当前目录有多少文件  # skip configured AI summary once / 本次跳过已配置的 AI summary
aicmd --summary 当前目录有多少文件     # request AI summary once / 本次请求 AI summary
aicmd --no-cache 当前目录有多少文件    # bypass successful command cache / 不复用缓存命令
aicmd "安装 jq，并验证安装结果"          # workflow may be selected automatically / 可能自动选择 workflow
```

### 7.1 Workflow execution / Workflow 执行

When a task needs environment checks, one or more changes, and final verification, AICmd can automatically select `workflow` from the ordinary `aicmd <task>` entry. `workflow` is not a new command.

当任务需要环境检查、一个或多个修改步骤和最终验证时，AICmd 可以从普通的 `aicmd <任务>` 入口自动选择 `workflow`。`workflow` 不是新命令。

```bash
aicmd "安装 jq，并验证安装结果"
```

Read-only checks run automatically. File and system changes run only after the complete workflow plan is confirmed. Modification steps are never retried automatically. A workflow is complete only after its read-only verification succeeds.

只读检查会自动运行。文件和系统修改只有在完整 workflow 计划确认后才会执行。修改步骤绝不自动重试；只有只读验证成功后，workflow 才算完成。

Only a leading contiguous prefix of `check` steps runs before any required confirmation. When the remaining eligible plan changes files or the system, AICmd shows that whole change plan once; destructive steps retain a second confirmation. A pure read-only original workflow may continue without a confirmation prompt. A repair plan is displayed and confirmed again, even when it contains only read-only work.

只有连续且位于开头的 `check` 步骤会在任何需要的确认前运行。当剩余符合条件的计划会修改文件或系统时，AICmd 会一次展示完整修改计划；破坏性步骤仍需二次确认。纯只读的原始 workflow 可以不显示确认提示而继续执行。修订计划即使只包含只读工作，也会再次展示并要求确认。

Workflow plans use the existing structured-plan JSON with `mode: "workflow"`; the workflow-only payload is `summary` plus `steps`:

workflow 计划使用既有结构化计划 JSON，并设置 `mode: "workflow"`；workflow 专属载荷为 `summary` 和 `steps`：

```json
{
  "mode": "workflow",
  "command": "",
  "query": "",
  "problem": "",
  "preflight": [],
  "summary": "Install jq and verify it",
  "steps": [
    {
      "id": "check-jq",
      "kind": "check",
      "command": "command -v jq",
      "risk": "read_only",
      "on_failure": "continue"
    },
    {
      "id": "install-jq",
      "kind": "action",
      "command": "<package-manager> install jq",
      "risk": "changes_system",
      "run_if": { "step": "check-jq", "result": "failed" },
      "on_failure": "stop"
    },
    {
      "id": "verify-jq",
      "kind": "verify",
      "command": "jq --version",
      "risk": "read_only",
      "on_failure": "repair"
    }
  ]
}
```

Each step has `id`, `kind`, `command`, `risk`, and `on_failure`; `run_if` is optional and can refer only to an earlier `check` result (`passed` or `failed`). `check` and `verify` must be read-only, and a workflow must contain at least one `verify`. Actions and verification run in plan order after any required confirmation.

每个步骤包含 `id`、`kind`、`command`、`risk` 和 `on_failure`；`run_if` 可选，且只能引用更早的 `check` 结果（`passed` 或 `failed`）。`check` 和 `verify` 必须只读，workflow 至少包含一个 `verify`。在任何需要的确认完成后，action 和验证会按计划顺序执行。

`Ctrl-C` stops the current step and all later steps, preserves captured output, writes one aggregate session record with the plan and ordered results, and exits 130. At most two repair plans are generated; each is strictly validated, rechecked, and reconfirmed. AI summary remains optional and is outside workflow completion status.

`Ctrl-C` 会停止当前步骤和之后的全部步骤，保留已捕获输出，写入一条包含计划和按序结果的聚合 session 记录，并以 130 退出。最多生成两份修订计划；每份都会严格校验、重新检查并重新确认。AI summary 仍是可选项，不参与 workflow 完成状态。

## 8. Successful command cache / 成功命令缓存

AICmd stores successful regular commands in:

AICmd 会把成功执行过的普通命令缓存到：

```text
~/.aicmd/command-cache.yaml
```

When the same task, shell, and OS match later, AICmd reuses the command and shows the regular confirmation:

后续如果同一个任务、shell 和 OS 匹配，AICmd 会复用该命令并显示普通确认：

```text
Reusing a previously successful command / 正在复用之前成功执行过的命令
```

Press `?`, then `g`, to generate a new command. / 输入 `?` 后再输入 `g` 可重新生成命令。

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

If the failure menu was closed, use the deterministic continuation phrase:

如果已经退出失败菜单，可以使用固定的继续表达：

```bash
aicmd continue fixing the last failed task
aicmd 继续修复刚才失败的任务
```

The failed command, exit code, stdout, and stderr are saved as an execution note. The phrase selects today's daily named session, bypasses successful-command cache lookup, and goes through the normal planner. The planner request includes its current system prompt and examples, recent non-system session history, and the current continuation request. If planning routes to command generation, that request uses the command-generation role's current system prompt and the same recent history. Old session system prompts are not reused. Confirmation is still required before any generated repair runs.

失败命令、exit code、stdout 和 stderr 会保存为执行记录。该表达会选择当天每日命名会话、绕过成功命令缓存，并进入正常 planner。planner 请求由当前角色的 system prompt 和 examples、最近的非 system 会话历史、以及当前继续请求组成；如果规划结果进入命令生成，后续请求会使用命令生成角色自己的当前 system prompt 和同一份最近历史，不会复用旧会话的 system prompt。任何生成的修复命令仍需确认后才执行。

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

`--from-search` requires both the saved summary and its raw MCP record. Before model generation, the raw record must contain at least one `http://` or `https://` URL and one recognized command-evidence line. If either signal is missing, AICmd names the insufficient saved search and recommends `aicmd search <more specific query>`.

`--from-search` 同时要求保存的摘要和对应原始 MCP 记录。调用模型前，原始记录必须至少包含一个 `http://` 或 `https://` URL，以及一行可识别的命令证据。缺少任一信号时，AICmd 会指出证据不足的搜索记录，并建议执行 `aicmd search <more specific query>`。

Passing this gate does not prove that a command is correct. It only blocks clearly incomplete evidence. After validation, summary and raw evidence are both added to context, and the model is instructed not to invent or repair missing URLs or commands from memory.

通过门槛不代表命令一定正确；它只拦截明显不完整的证据。验证通过后，摘要和原始证据都会进入上下文，模型也会被明确要求不得依靠记忆补全或修复缺失的 URL 与命令。

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

Equivalent natural-language forms / 等效的自然语言表达：

```bash
aicmd 保存刚才的搜索结果为 docker-install
aicmd 用刚才的搜索结果安装 Docker
aicmd save the last search as docker-install
aicmd use the last search result to install Docker
```

`aicmd mcp-raw <command> ...` prints raw MCP output and is mainly for debugging.

`aicmd mcp-raw <command> ...` 会打印 MCP 原始输出，主要用于调试。

## 12. Sessions / 会话

AICmd saves one-shot normal commands to daily history by default, such as `cmd-20260712`, but does not send that history to the model. The no-argument continuous prompt explicitly uses that daily session for every child task.

AICmd 默认把单次普通命令保存到每日 history，例如 `cmd-20260712`，但不会把这些历史发送给模型。不带参数的连续交互会为每个子任务显式使用该每日会话。

Common natural-language actions:

常用自然语言操作：

```bash
aicmd show current session
aicmd list sessions
aicmd show last 5 messages in session dev
aicmd in session dev continue with the previous task
aicmd clear session dev

aicmd 查看当前会话
aicmd 列出所有会话
aicmd 查看 dev 最近 5 条对话
aicmd 在 dev 会话中继续处理刚才的问题
aicmd 清空 dev 会话
```

Named-session use affects only the current invocation. It enables continuing context for that task, but it does not persistently switch the default; a later plain `aicmd <task>` writes to the Beijing-date daily session again.

命名会话只影响当前这次调用，并为该任务启用连续上下文；它不会持久切换默认会话。之后普通的 `aicmd <任务>` 仍写入按北京时间命名的每日会话。

Clearing either the current or a named session always resolves and displays the target session name, then asks for confirmation. Cancelling leaves the session file unchanged.

清空当前会话或命名会话时，AICmd 一定会解析并显示目标会话名，然后要求确认；取消后会话文件保持不变。

Explicit `-s`/`--session`, `--empty-session`, and `--list-sessions` controls take precedence over natural-language session intent. The explicit CLI option determines what happens next.

显式的 `-s`/`--session`、`--empty-session` 和 `--list-sessions` 优先于自然语言会话意图，后续行为由显式 CLI 参数决定。

No session is loaded or modified before clear confirmation. After confirmation, a missing target reports `Session not found` and no empty session file is created.

清空确认前不会加载或修改任何目标会话。确认后若目标不存在，会报 `Session not found`，且不会创建空会话文件。

### 12.1 Supported natural-language forms / 支持的自然语言表达

The parser intentionally accepts only the forms below. `<N>` must be a positive integer. `<name>` is a session or saved-search name, and `<task>` must not be empty.

解析器只接受下表中的表达。`<N>` 必须是正整数，`<name>` 是会话名或搜索记录名，`<task>` 不能为空。

| Action / 操作 | Chinese forms / 中文表达 | English forms / 英文表达 | Behavior / 行为 |
| --- | --- | --- | --- |
| Continue last failure / 继续上次失败 | `继续修复刚才失败的任务`<br>`继续处理刚才失败的任务` | `continue fixing the last failed task`<br>`continue the last failed task` | Selects today's daily session, bypasses command cache, and returns to normal planning and confirmation. / 选择当天每日会话、绕过命令缓存，并回到正常规划与确认流程。 |
| Save last search / 保存最近搜索 | `保存刚才的搜索结果`<br>`保存最近的搜索结果`<br>`保存刚才的搜索结果为 <name>`<br>`保存最近的搜索结果为 <name>`<br>`保存刚才的搜索结果，命名为 <name>`<br>`保存最近的搜索结果，命名为 <name>` | `save the last search`<br>`save the last search result`<br>`save the last search as <name>`<br>`save the last search result as <name>` | Saves the latest search; an omitted name uses the existing automatic naming flow. / 保存最近搜索；省略名称时使用现有自动命名流程。 |
| Use last search / 使用最近搜索 | `用刚才的搜索结果<task>`<br>`使用刚才的搜索结果<task>`<br>`根据刚才的搜索结果<task>`<br>`用最近的搜索结果<task>` | `use the last search result to <task>`<br>`use the last search to <task>` | Enters the existing `do` flow and still requires command review and confirmation. / 进入现有 `do` 流程，仍需检查并确认生成命令。 |
| Show recent daily-session messages / 查看每日会话最近消息 | `查看最近 <N> 条对话`<br>`查看最近 <N> 条上下文`<br>`查看最近 <N> 条消息` | `show last <N> context messages`<br>`show last <N> messages`<br>`show recent <N> messages` | Shows up to `<N>` non-system messages from today's Beijing-date session. / 最多显示北京时间当天会话中的 `<N>` 条非 system 消息。 |
| Show current session / 查看当前会话 | `查看当前会话` | `show current session` | Resolves the Beijing-date daily session. / 解析北京时间当天的每日会话。 |
| List sessions / 列出会话 | `列出所有会话`<br>`列出会话` | `list sessions` | Lists saved session files. / 列出已保存的会话文件。 |
| Show named-session messages / 查看命名会话消息 | `查看 <name> 最近 <N> 条对话`<br>`查看 <name> 最近 <N> 条消息`<br>`查看 <name> 最近 <N> 条上下文` | `show last <N> messages in session <name>` | Shows up to `<N>` non-system messages; the English form requires a one-word session name. / 最多显示 `<N>` 条非 system 消息；英文表达要求会话名是一个单词。 |
| Clear current session / 清空当前会话 | `清空当前会话` | `clear current session` | Shows the resolved daily-session name and asks for confirmation. / 显示解析后的每日会话名并要求确认。 |
| Clear named session / 清空命名会话 | `清空 <name> 会话` | `clear session <name>` | Shows the named target and asks for confirmation. / 显示命名目标并要求确认。 |
| Run in named session / 在命名会话中运行 | `在 <name> 会话中<task>` | `in session <name> <task>` | Uses continuing context for this invocation only; the English form requires a one-word session name. / 仅在本次调用中使用连续上下文；英文表达要求会话名是一个单词。 |

These forms do not implement persistent session switching, fuzzy intent matching, or additional aliases. If a phrase does not match, AICmd continues through the normal command-planning path.

这些表达不提供持久会话切换、模糊意图匹配或额外别名。如果表达没有匹配，AICmd 会继续走普通命令规划流程。

### 12.2 Advanced explicit commands / 高级显式命令

The existing explicit session commands remain available:

现有显式会话命令仍然可用：

```bash
aicmd -s                       # show current/default session / 显示当前默认会话
aicmd -s dev                   # use/create dev for this invocation / 本次调用使用或创建 dev 会话
aicmd -s dev hello             # send request in dev session / 在 dev 会话中发送请求
aicmd --list-sessions          # list sessions / 列出会话
aicmd -s dev --empty-session   # clear dev session with confirmation / 二次确认后清空 dev 会话
```

Use `-s <name>` when you want continuing context for one invocation. / 需要在单次调用中使用连续上下文时再使用 `-s <名称>`。

History commands:

历史命令：

```bash
aicmd session
aicmd session list
aicmd session show
aicmd session show dev --limit 5
aicmd 查看最近 5 条上下文
aicmd show last 5 messages
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

### Long-running task status and retries / 长时间任务状态与重试

Model and MCP work shares one request budget: at most 3 attempts, 15 seconds per attempt, and approximately 45 seconds overall. Terminal output shows the stage, attempt number, elapsed time, and `Ctrl-C` hint. Non-TTY use writes status transitions to stderr and keeps stdout suitable for pipelines.

模型和 MCP 工作共享同一个请求额度：最多尝试 3 次、每次最多 15 秒、全流程总计约 45 秒。终端会显示阶段、尝试次数、耗时和 `Ctrl-C` 提示。非 TTY 模式只把状态切换写入 stderr，stdout 仍可用于管道。

Only timeouts, transport interruptions, HTTP 429/5xx, and unexpected MCP process/channel exits are retried. Authentication, request, sensitive-content, MCP configuration, and tool-selection errors stop immediately. Every MCP retry starts a fresh server process and reaps the previous child.

只有超时、传输中断、HTTP 429/5xx，以及 MCP 进程或响应通道意外退出会重试。鉴权、请求、敏感内容、MCP 配置和工具选择错误会立即停止。每次 MCP 重试都会启动新的 server 进程并回收上一次子进程。

Confirmed shell commands are not part of this retry policy. They have no default timeout and are never executed again automatically. When a command is quiet for 5 seconds, AICmd prints a liveness update. Cancelling with `Ctrl-C` waits briefly for SIGINT, then terminates the shell process if needed, preserves partial stdout/stderr, and stores exit code 130 plus `Termination: cancelled` in the session.

已经确认的 shell 命令不属于自动重试范围。它们没有默认超时，也不会被自动再次执行。命令静默运行 5 秒后，AICmd 会显示存活状态。按 `Ctrl-C` 取消时，AICmd 会短暂等待 SIGINT；必要时终止 shell 进程，保留已有 stdout/stderr，并把退出码 130 和 `Termination: cancelled` 保存到 session。

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

For commands already classified as `ChangesSystem` or `Destructive`, AICmd captures Git porcelain status before and after execution, including failed execution. In a Git worktree, it prints only new records or records whose status changed:

对于现有风险分类为 `ChangesSystem` 或 `Destructive` 的命令，AICmd 会在执行前后捕获 Git porcelain status，包括执行失败的情况。在 Git 工作区中，只显示新增记录或状态发生变化的记录：

```text
Detected file changes: / 检测到文件变化：
- M src/main.rs
- ?? output/report.txt
```

The report recommends `git diff` and manual recovery. AICmd does not run `git reset`, delete files, or perform rollback. Git capture is advisory and silently skips non-Git directories or capture failures.

报告会建议使用 `git diff` 并手动恢复。AICmd 不会执行 `git reset`、删除文件或自动回滚。Git 捕获只是提示；非 Git 目录或捕获失败时会静默跳过。

## 19. Troubleshooting / 排障

`aicmd doctor` performs offline checks for binary path, version, config, model, temperature, AI summary, command cache, saved searches directory, PATH, shell integration, and MCP configuration. MCP checks validate JSON, command-to-server references, `stdio` server type, non-empty commands, executable availability through a path or `PATH`, and optional tool fields. Doctor does not start MCP servers or access the network.

`aicmd doctor` 会执行离线检查，包括二进制路径、版本、配置、模型、temperature、AI summary、命令缓存、搜索记录目录、PATH、shell 集成和 MCP 配置。MCP 检查会验证 JSON、command 到 server 的引用、`stdio` server 类型、非空 command、通过路径或 `PATH` 查找可执行文件，以及可选 tool 字段。Doctor 不会启动 MCP server 或联网。

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

Runtime errors identify the failed stage: `start`, `initialize`, `tools/list`, `tool selection`, or `tools/call`. Follow the included action. Only a structured local response timeout recommends increasing `AICMD_MCP_START_TIMEOUT_SECS` or `AICMD_MCP_CALL_TIMEOUT_SECS`; other stage errors recommend `aicmd doctor` or correcting executable, arguments, or tool selection.

运行时错误会标明失败阶段：`start`、`initialize`、`tools/list`、`tool selection` 或 `tools/call`。请按错误中的建议操作。只有结构化的本地响应超时才会建议增大 `AICMD_MCP_START_TIMEOUT_SECS` 或 `AICMD_MCP_CALL_TIMEOUT_SECS`；其他阶段错误会建议运行 `aicmd doctor`，或修正可执行文件、参数、tool 选择。

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
