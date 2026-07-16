# AICmd Session Natural-Language Intents Design
# AICmd 会话自然语言意图设计

## Summary / 摘要

English: Extend AICmd's existing deterministic natural-language intent parser with a small set of session operations. Recognized phrases reuse the current `session` and `-s` execution paths without a model call. Unmatched text continues through the structured planner.

中文：扩展 AICmd 现有的确定性自然语言意图解析器，增加少量会话操作。识别成功后直接复用当前 `session` 和 `-s` 执行路径，不调用模型；未匹配文本继续进入结构化规划器。

## Goals / 目标

- Show the current daily session through natural language. / 通过自然语言查看当前每日会话。
- List saved sessions. / 列出已保存会话。
- Show the latest N messages from a named session. / 查看指定会话最近 N 条消息。
- Clear the current or a named session with the existing confirmation. / 使用现有确认流程清空当前或指定会话。
- Run one task in a named session with context enabled. / 在指定命名会话中执行一次任务并启用上下文。
- Preserve the daily session as the default for later plain `aicmd` calls. / 后续普通 `aicmd` 调用仍默认使用每日会话。

## Non-Goals / 非目标

- Do not persist a new default session. / 不保存新的持久默认会话。
- Do not implement `switch to session dev` because "switch" implies persistent state. / 不实现“切换到 dev 会话”，因为“切换”暗示持久状态。
- Do not add a new CLI command, configuration field, state file, dependency, TUI, or model call. / 不增加 CLI 命令、配置字段、状态文件、依赖、TUI 或模型调用。
- Do not broaden matching into a general natural-language router. / 不扩展为通用自然语言路由器。

## Supported Intents / 支持的意图

| Intent / 意图 | Chinese / 中文 | English / 英文 | Existing behavior reused / 复用行为 |
| --- | --- | --- | --- |
| Current session / 当前会话 | `查看当前会话` | `show current session` | `aicmd session` |
| List sessions / 列出会话 | `列出所有会话` | `list sessions` | `aicmd session list` |
| Show recent messages / 查看最近消息 | `查看 dev 最近 5 条对话` | `show last 5 messages in session dev` | `aicmd session show dev --limit 5` |
| Clear current / 清空当前会话 | `清空当前会话` | `clear current session` | Daily session plus `--empty-session` |
| Clear named / 清空命名会话 | `清空 dev 会话` | `clear session dev` | `aicmd -s dev --empty-session` |
| Run in named session / 在命名会话中执行 | `在 dev 会话中继续处理这个问题` | `in session dev continue with this task` | `aicmd -s dev <task>` |

Only the listed low-ambiguity forms and their documented spacing variants are recognized. Similar but unmatched text falls through to the existing planner.

只识别上述低歧义句式及文档明确支持的空格变体。相似但未匹配的文本继续进入现有规划器。

## Architecture / 架构

### Intent parser / 意图解析器

Extend `src/intent_cmd.rs` with focused variants for current session, session list, named-session history, clear current/named session, and one-task named-session execution. The parser remains pure and returns typed data such as session name, limit, and task.

扩展 `src/intent_cmd.rs`，增加当前会话、会话列表、命名会话历史、清空当前/命名会话，以及在命名会话中执行一次任务的类型。解析器保持纯函数，只返回 session 名称、数量和任务等结构化数据。

### Read-only routing / 只读路由

Current-session, list, and show-history intents run before runtime model initialization and call `session_cmd::run_session_command` with the equivalent existing arguments.

当前会话、列表和历史查看意图在运行时模型初始化前执行，并使用等效参数调用 `session_cmd::run_session_command`。

### Session task routing / 会话任务路由

For `in session <name> <task>`, translate the parsed intent into the existing `Cli.session = Some(Some(name))` and task text before entering `run`. This preserves session creation, context loading, command planning, confirmation, execution, and result persistence in one shared path.

对于“在 `<name>` 会话中 `<task>`”，在进入 `run` 前将意图转换为现有的 `Cli.session = Some(Some(name))` 和任务文本。这样可以在同一共享路径中保留 session 创建、上下文加载、命令规划、确认、执行和结果记录。

The session override applies only to the current process. A later plain `aicmd <task>` still uses `cmd-YYYYMMDD` in Beijing time.

会话覆盖只对当前进程有效。下一次普通 `aicmd <task>` 仍使用北京时间对应的 `cmd-YYYYMMDD`。

### Clear routing / 清空路由

Translate clear intents into the existing `empty_session` flow. `clear current session` resolves to the Beijing-date daily session; `clear session <name>` resolves to the named session. The existing high-risk confirmation must display the resolved session name before any data is changed.

将清空意图转换为现有 `empty_session` 流程。“清空当前会话”解析为北京时间每日会话；“清空 `<name>` 会话”解析为命名会话。修改数据前必须由现有高风险确认流程显示最终 session 名称。

## Validation And Errors / 校验与错误

- Message limits must be positive integers. / 消息数量必须是正整数。
- A named-session task must include both a session name and non-empty task. / 命名会话任务必须同时包含 session 名称和非空任务。
- Incomplete recognized phrases fail with an actionable message and do not call the model. / 已识别但不完整的句式返回可执行提示，不调用模型。
- Showing a missing named session returns the existing not-found error. / 查看不存在的命名会话时返回现有未找到错误。
- Running in a missing named session creates it through existing `-s` behavior. / 在不存在的命名会话中执行任务时，通过现有 `-s` 行为自动创建。
- Canceling clear leaves the session file unchanged. / 取消清空后 session 文件保持不变。
- Existing session-name rules and path handling remain authoritative; this feature does not add a second validator. / 现有 session 名称规则和路径处理仍是唯一标准，本功能不增加第二套校验器。

## Testing / 测试

Unit tests cover Chinese and English parsing, session-name and limit extraction, incomplete phrases, zero limits, and non-session terminal tasks that must not match.

单元测试覆盖中英文解析、session 名称和数量提取、不完整句式、数量为零，以及不得误匹配的普通终端任务。

Manual tests use temporary session directories and verify:

真实流程测试使用临时 session 目录，并验证：

1. Current session resolves to the Beijing-date daily name. / 当前会话解析为北京时间每日名称。
2. Saved sessions are listed. / 已保存会话能够列出。
3. Named history respects the requested limit. / 命名会话历史遵守数量限制。
4. A missing named session is created for a task and context is enabled. / 不存在的命名会话在执行任务时自动创建并启用上下文。
5. Current and named clear operations show the exact target, preserve data on cancellation, and clear only after confirmation. / 当前和命名清空操作显示准确目标，取消时保留数据，确认后才清空。
6. A later plain command still selects the daily session. / 后续普通命令仍选择每日会话。

Full verification requires `cargo fmt --check`, `cargo test`, `cargo clippy --all --all-targets -- -D warnings`, `cargo build --release`, and `git diff --check`.

完整验证包括 `cargo fmt --check`、`cargo test`、`cargo clippy --all --all-targets -- -D warnings`、`cargo build --release` 和 `git diff --check`。
