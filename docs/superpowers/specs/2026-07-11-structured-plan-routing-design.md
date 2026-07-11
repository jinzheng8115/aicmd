# AICmd Structured Plan Routing Design
# AICmd 结构化计划路由设计

## Goal / 目标

Replace user-selected execution modes with one natural-language entry point.
The model returns a validated structured plan, then AICmd routes it to command
execution, script execution, MCP search, or error diagnosis.

用一个自然语言入口替代用户手动选择执行模式。模型返回经过校验的结构化计划，
随后 AICmd 路由到命令执行、脚本执行、MCP 搜索或错误诊断。

## Scope / 范围

- Plain `aicmd <task>` automatically selects `direct`, `script`, `search`, or
  `diagnose`.
- Existing `aicmd do`, `aicmd search`, and `aicmd err` remain compatible as
  advanced entry points.
- Split the main interactive path into plan, confirmation, execution, and
  result responsibilities.
- Remove command-specific Markdown and natural-language cleanup heuristics.

- 普通 `aicmd <任务>` 自动选择 `direct`、`script`、`search` 或 `diagnose`。
- 现有 `aicmd do`、`aicmd search`、`aicmd err` 保持兼容，作为高级入口。
- 将主交互链拆分为计划、确认、执行、结果四项职责。
- 删除面向命令的 Markdown 与自然语言清洗启发式规则。

## Non-goals / 非目标

- Do not remove existing advanced commands.
- Do not add a TUI, a generic agent loop, or a new model provider.
- Do not change `.env`, `config.yaml`, `mcp.json`, session storage, or the
  existing high-risk confirmation policy.
- Do not remove terminal formatting for search answers or AI summaries; those
  are presentation output, not shell-command parsing.

- 不删除已有高级命令。
- 不新增 TUI、通用 Agent 循环或模型提供商。
- 不修改 `.env`、`config.yaml`、`mcp.json`、session 存储或已有高风险二次确认策略。
- 不删除搜索答案和 AI summary 的终端格式化；它们属于展示输出，而非 shell 命令解析。

## Structured plan contract / 结构化计划协议

The default shell role returns one JSON object and no surrounding text.

默认 shell role 只返回一个 JSON 对象，不附带其他文本。

```json
{
  "mode": "direct",
  "command": "du -sh ."
}
```

Supported modes / 支持模式：

| Mode | Required field | AICmd action |
| --- | --- | --- |
| `direct` | `command` | Confirm and execute one shell command. / 确认并执行一条 shell 命令。 |
| `script` | `command` | Confirm and execute a generated script wrapper. / 确认并执行生成的脚本包装命令。 |
| `search` | `query` | Invoke configured MCP search, then summarize the result. / 调用已配置的 MCP 搜索，再整理结果。 |
| `diagnose` | `problem` | Invoke the existing diagnosis path with the supplied error/problem text. / 使用给出的错误或问题文本进入现有诊断链。 |

`command`, `query`, and `problem` must be non-empty for their applicable mode.
The parser rejects unknown JSON fields. Unknown modes, invalid JSON, missing
fields, unknown fields, or extra non-whitespace output are invalid plans.

对应模式的 `command`、`query`、`problem` 必须非空。解析器拒绝未知 JSON 字段。未知模式、
非法 JSON、缺少字段、未知字段，或 JSON 外存在非空白文本，均视为无效计划。

## Invalid-plan behavior / 无效计划处理

Invalid plans do not run a command, do not invoke MCP, and do not fall back to
Markdown/code-fence/natural-language guessing. AICmd prints a bilingual error
and asks the user to retry or revise the task.

无效计划不执行命令、不调用 MCP，也不回退到 Markdown、代码块或自然语言猜测。AICmd
输出双语错误，并提示用户重试或修改任务。

This strict failure is intentional: it replaces hidden command rewrites with a
visible, safe boundary.

这种严格失败是有意设计：以可见且安全的边界取代隐藏的命令改写。

## Routing flow / 路由流程

```text
User task / 用户任务
  -> structured plan request / 请求结构化计划
  -> JSON validation / JSON 校验
  -> direct | script | search | diagnose
  -> confirmation where execution can change the system / 会修改系统时确认
  -> execution or MCP/diagnosis / 执行或 MCP/诊断
  -> result recording, cache, optional summary, repair / 结果记录、缓存、可选总结、修复
```

`direct` and `script` use the existing `Run? [Y/n/?]` confirmation. Existing
high-risk commands continue to require their second confirmation. `search` and
`diagnose` do not execute shell commands by routing alone.

`direct` 与 `script` 使用现有 `Run? [Y/n/?]` 确认。高风险命令仍需要二次确认。
仅靠路由进入 `search` 或 `diagnose` 不会执行 shell 命令。

## Code boundaries / 代码边界

Create focused internal modules while retaining the public CLI behavior:

| Module | Responsibility |
| --- | --- |
| `plan_cmd` | Build the planner request, parse and validate the JSON plan, expose the plan type. |
| `confirm_cmd` | Print a proposed command, risk hint, and `Y/n/?` advanced actions. |
| `execute_cmd` | Execute shell commands and capture exit code, stdout, stderr, cwd, and environment facts. |
| `result_cmd` | Record session notes, command-cache successes, optional summaries, and failure-repair follow-up. |

在保留公开 CLI 行为的同时新增聚焦的内部模块：

| 模块 | 职责 |
| --- | --- |
| `plan_cmd` | 构造计划请求、解析并校验 JSON 计划、暴露计划类型。 |
| `confirm_cmd` | 输出候选命令、风险提示和 `Y/n/?` 高级操作。 |
| `execute_cmd` | 运行 shell 命令并捕获退出码、stdout、stderr、cwd 和环境事实。 |
| `result_cmd` | 记录 session、成功命令缓存、可选 summary 和失败修复后续操作。 |

`main.rs` remains responsible for process startup, existing explicit-command
shortcuts, and top-level routing only. It must not parse commands from prose.

`main.rs` 只负责进程启动、已有显式命令快捷入口和顶层路由；不得再从自然语言中解析命令。

## Heuristic removal / 启发式删除

Delete the command sanitization chain, including Markdown fence removal,
prose-line filtering, provider-marker stripping, and command-specific rewrite
rules. Delete the tests that only assert those rewrites.

删除命令清洗链，包括 Markdown 围栏移除、自然语言行过滤、提供商标记剥离和命令专用改写
规则。删除只验证这些改写的测试。

Keep shell risk classification, shell quoting, output decoding, and terminal
presentation helpers because they serve execution safety or output display.

保留 shell 风险分类、shell 引号处理、输出解码和终端展示 helper，因为它们服务于执行安全或
输出显示。

## Documentation and help / 文档与帮助

README and `aicmd help` present the ordinary workflow as:

```text
aicmd <task>
```

They explain that AICmd chooses the appropriate operation automatically.
`do`, `search`, and `err` remain listed in the advanced reference for users
who need an explicit mode.

README 和 `aicmd help` 将普通工作流呈现为 `aicmd <任务>`，并说明 AICmd 会自动选择合适
操作。`do`、`search`、`err` 保留在高级参考中，供需要显式模式的用户使用。

## Verification / 验证

- Unit-test valid and invalid plans, including each required field.
- Unit-test that invalid plans do not reach command execution or MCP routing.
- Test direct/script routing without a live model by injecting parsed plans.
- Test search/diagnose routing with existing helper seams.
- Run `cargo fmt --check`, `cargo test`, `cargo clippy --all --all-targets -- -D warnings`, `cargo build`, and `git diff --check`.
- Manually verify `aicmd help` and one direct, script, search, and diagnosis request using the configured model/MCP setup.

- 为有效和无效计划（含每种必填字段）添加单元测试。
- 验证无效计划不会进入命令执行或 MCP 路由。
- 通过注入已解析计划测试 direct/script 路由，不依赖在线模型。
- 使用现有 helper 接缝测试 search/diagnose 路由。
- 运行 `cargo fmt --check`、`cargo test`、`cargo clippy --all --all-targets -- -D warnings`、`cargo build`、`git diff --check`。
- 使用已配置模型/MCP 手动验证 direct、script、search、diagnose 各一次，并检查 `aicmd help`。
