# AICmd P0-P2 Interaction, Recovery, and Diagnostics Design

## Summary / 概要

AICmd will make the default path more useful without adding commands users must memorize:

- P0: running `aicmd` with no text enters a continuous terminal prompt.
- P1: the prompt can continue a failed task from session context, and search-to-execution refuses weak evidence.
- P2: file-changing commands report Git-visible changes and recovery guidance, while existing `doctor` and MCP errors provide actionable diagnostics.

AICmd 将增强默认入口，但不增加用户必须记忆的新命令：

- P0：不带文本运行 `aicmd` 时进入连续终端交互。
- P1：交互中可以根据 session 上下文继续失败任务；搜索转执行时拒绝证据不足的结果。
- P2：修改文件的命令显示 Git 可见变化和恢复建议；现有 `doctor` 与 MCP 错误提供可操作诊断。

## Goals / 目标

1. A user can run `aicmd` once and submit multiple tasks.
2. Every interactive task uses the Beijing-date daily session with context enabled.
3. `exit`, `quit`, `.exit`, EOF, and Ctrl-C leave the prompt without changing shell state.
4. A failed command remains in session history, so a later prompt such as `继续修复刚才失败的任务` can use its command, exit code, stdout, and stderr.
5. Search-based execution uses both the saved summary and raw MCP result.
6. Search-based execution that lacks a source URL or executable-looking command evidence must stop before asking the model to generate a modifying script.
7. Commands that may modify files capture Git status before and after execution and print changed paths plus safe inspection guidance.
8. `aicmd doctor` validates MCP configuration and executable availability without starting servers.
9. MCP runtime failures identify the failed stage and include one concrete next action.

## Non-goals / 非目标

- No full-screen TUI, command history editor, autocomplete, or new dependency.
- No persistent default named session.
- No automatic `git reset`, file deletion, package uninstall, or rollback.
- No network probe in `doctor`.
- No new `retry`, `rollback`, or `mcp-doctor` command.
- No second model call solely to validate search evidence.

不实现全屏 TUI、自动补全、持久默认命名会话、自动回滚、联网诊断或新的辅助子命令。

## P0: Continuous Prompt / 连续交互

### Entry behavior / 入口行为

No arguments plus an interactive terminal starts the prompt:

```text
AICmd 0.4.2
Session: cmd-20260712
输入任务，exit 退出。

AICmd> 查看内存占用最高的程序
```

No arguments from a pipe or redirected stdin keep the existing non-interactive behavior and do not wait for input.

### Execution model / 执行模型

The prompt is a small parent loop. Each non-empty line starts the current AICmd executable as a child process with:

```text
-s cmd-YYYYMMDD <line>
```

The child inherits stdin, stdout, stderr, current directory, and environment. This reuses the existing planner, search routing, confirmation UI, repair loop, execution-result persistence, and language setting. The parent waits for the child and then displays the next prompt even if the child exits non-zero.

The parent does not parse shell words. The whole line is passed as one text argument, avoiding quoting changes.

### Session semantics / 会话语义

The daily session name is resolved once when the prompt starts. Every child call explicitly enables that session context. Named-session natural-language forms can still be entered, but only affect their child invocation under existing precedence rules.

## P1: Recovery and Search Evidence / 恢复与搜索证据

### Failed-task continuation / 失败任务继续

No new persisted failure file is needed. Existing execution notes already store:

- command
- exit code
- stdout
- stderr
- optional AI summary

Interactive child calls use the same context-enabled daily session. Therefore a later natural-language request can reference the previous failure. Add one deterministic intent:

```text
继续修复刚才失败的任务
continue fixing the last failed task
```

It routes to the daily session with context enabled and uses the exact text as the task. It does not execute automatically; the normal planner and confirmation flow remain in control.

### Search evidence gate / 搜索证据门槛

For every `do --from-search <name>`:

1. Require the saved summary.
2. Require the corresponding raw search record.
3. Read both into the model context.
4. Inspect the raw result locally for:
   - at least one `http://` or `https://` source;
   - at least one line whose trimmed content starts with `$ ` or one of `brew `, `npm `, `npx `, `pnpm `, `yarn `, `apt `, `apt-get `, `dnf `, `yum `, `pacman `, `curl `, `wget `, `pip `, `pip3 `, `python `, `python3 `, `cargo `, `go `, `git `, `docker `, `kubectl `, `sh `, `bash `, or `zsh `.
5. If either signal is missing, return an error explaining that the search result is insufficient and recommend running a more specific search.

The local gate is intentionally conservative. It does not decide whether a command is correct; it only blocks clearly incomplete evidence.

The search-based prompt must state that URLs and commands may not be invented or repaired from memory. If source evidence is incomplete, the generated command must be a safe explanation command that makes no system changes.

## P2: Change Recovery Guidance / 变更恢复提示

### Git snapshot / Git 状态快照

Before executing commands whose existing risk classifier reports file/system modification:

1. Detect whether the current directory is inside a Git worktree.
2. Capture `git status --porcelain=v1 --untracked-files=all`.
3. Execute the command normally.
4. Capture status again.
5. Print paths whose status line was added or changed.

Example:

```text
检测到文件变化：
- M src/main.rs
- ?? output/report.txt

恢复建议：
1. 使用 git diff 查看已跟踪文件变化。
2. 确认后再手动恢复；AICmd 不会自动删除或重置文件。
```

If the directory is not a Git worktree or status capture fails, execution continues without a recovery section.

## P2: MCP Diagnostics / MCP 诊断

### Doctor checks / Doctor 检查

`aicmd doctor` extends its existing MCP check:

- configuration file exists and parses;
- every configured command references an existing server;
- server type is `stdio`;
- server has a non-empty `command`;
- the executable is available through an absolute path or `PATH`;
- optional configured tool is a non-empty string.

It does not start the MCP server or call the network.

### Runtime stages / 运行阶段

Runtime errors use these stage labels:

- `start`: process could not start;
- `initialize`: initialization failed or timed out;
- `tools/list`: tool discovery failed or timed out;
- `tool selection`: no suitable tool could be selected;
- `tools/call`: tool execution failed or timed out.

Each error includes the server/command name where available and one next action:

- verify executable and arguments;
- run `aicmd doctor`;
- configure an explicit tool only when automatic discovery cannot choose;
- increase the existing timeout environment variable when the server is merely slow.

## Error Handling / 错误处理

- Interactive child failure does not terminate the parent prompt.
- EOF and exit words leave with status 0.
- Failure to start the current executable ends the prompt with an error.
- Search evidence failure happens before model invocation and before confirmation.
- Git status capture is advisory and never blocks command execution.
- MCP diagnostics never print secret environment values.

## Testing / 测试

- Unit-test prompt exit recognition and child argument construction.
- Run an interactive smoke test with a temporary fake child executable.
- Unit-test failed-task intent parsing and routing.
- Unit-test search evidence with URL/command present and each signal absent.
- Unit-test search context includes both summary and raw record.
- Unit-test Git status delta calculation.
- Unit-test MCP configuration diagnostics with missing server, missing executable, unsupported type, and valid configuration.
- Unit-test MCP stage error formatting without secrets.
- Run formatting, full tests, Clippy with warnings denied, release build, and installed-binary smoke tests.

## Compatibility / 兼容性

- Existing text invocations and explicit subcommands are unchanged.
- `aicmd --help` still prints help.
- Non-terminal no-argument invocation does not enter the prompt.
- macOS and Linux remain supported; Windows remains unsupported.
- No new dependency or persistent configuration field is introduced.
