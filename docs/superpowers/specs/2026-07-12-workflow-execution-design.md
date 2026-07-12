# AICmd Workflow Execution Design / AICmd 连续任务执行设计

## Purpose / 目的

AICmd should complete multi-step terminal tasks without requiring the user to choose and re-enter every step. It keeps the existing human-in-the-loop safety model: read-only checks may run automatically, while any file or system change requires one confirmation of the complete plan.

AICmd 应能连续完成多步骤终端任务，用户不需要逐步选择或重复描述。系统继续保留人工确认边界：只读检查可以自动执行，任何文件或系统修改都必须先展示完整计划并确认一次。

## Scope / 范围

This design adds a `workflow` plan mode for tasks that require checks, changes, and verification. Existing `direct`, `script`, `search`, and `diagnose` modes remain unchanged. Explicit `do`, `search`, and `err` commands remain advanced entry points.

本设计新增 `workflow` 计划模式，用于包含检查、修改和验证的任务。现有 `direct`、`script`、`search`、`diagnose` 保持不变；显式的 `do`、`search`、`err` 继续作为高级入口。

The first version does not add a generic autonomous agent loop, parallel step execution, background jobs, or user-configurable retry policies.

首个版本不增加通用自主 Agent 循环、并行步骤、后台任务或用户可配置的重试策略。

## Structured plan / 结构化计划

The planner returns strict JSON. Markdown fences, surrounding prose, unknown fields, duplicate step IDs, empty commands, invalid risk values, and invalid dependencies are rejected before execution.

Planner 必须返回严格 JSON。Markdown 代码块、额外说明、未知字段、重复步骤 ID、空命令、非法风险值和无效依赖都会在执行前被拒绝。

```json
{
  "mode": "workflow",
  "summary": "Install and verify a tool",
  "steps": [
    {
      "id": "check",
      "kind": "check",
      "command": "command -v tool",
      "risk": "read_only",
      "on_failure": "continue"
    },
    {
      "id": "install",
      "kind": "action",
      "command": "brew install tool",
      "risk": "changes_system",
      "run_if": {
        "step": "check",
        "result": "failed"
      },
      "on_failure": "stop"
    },
    {
      "id": "verify",
      "kind": "verify",
      "command": "tool --version",
      "risk": "read_only",
      "on_failure": "repair"
    }
  ]
}
```

Step kinds are:

- `check`: a read-only environment or state check.
- `action`: a command that completes part of the task.
- `verify`: a read-only command that proves the requested result.

`run_if` is optional and may reference only an earlier `check` step. Its `result` is `passed` or `failed`. This deliberately avoids arbitrary expressions: for example, installation runs only when the preceding existence check failed. A step without `run_if` always runs when execution reaches it.

步骤类型：

- `check`：只读环境或状态检查。
- `action`：完成任务一部分的命令。
- `verify`：证明任务结果的只读命令。

`run_if` 为可选字段，只能引用之前的 `check` 步骤；`result` 只能是 `passed` 或 `failed`。首个版本不支持任意条件表达式，例如安装步骤只在前面的存在性检查失败时运行。没有 `run_if` 的步骤在流程到达时正常执行。

Risk values are `read_only`, `changes_files`, `changes_system`, and `destructive`. `on_failure` values are `continue`, `stop`, and `repair`. Local command-risk analysis may raise the declared risk but never lower it.

风险值为 `read_only`、`changes_files`、`changes_system` 和 `destructive`。`on_failure` 值为 `continue`、`stop` 和 `repair`。本地命令风险分析可以提升模型声明的风险等级，但不能降低。

## Execution flow / 执行流程

1. Parse and validate the complete plan.
2. Run only `read_only` checks before confirmation.
3. Use check results to skip unnecessary steps, such as installation when the tool already exists.
4. Display the final plan if any file or system changes remain.
5. Confirm the complete plan once. Any later plan change requires a new confirmation.
6. Execute steps in order and record every result.
7. Run the final read-only verification automatically.
8. Mark the workflow complete only when verification succeeds.

执行流程：

1. 解析并校验完整计划。
2. 确认前只运行 `read_only` 检查。
3. 根据检查结果跳过不必要步骤，例如工具已存在时跳过安装。
4. 如果仍包含文件或系统修改，展示最终完整计划。
5. 用户只确认一次；后续计划只要发生变化就必须重新确认。
6. 按顺序执行步骤，并记录每一步结果。
7. 自动运行最终只读验证。
8. 只有验证成功才将 workflow 标记为完成。

Read-only workflows need no confirmation. Commands that modify files or the system never run before confirmation. Destructive commands retain the existing high-risk second confirmation.

纯只读 workflow 不需要确认。修改文件或系统的命令在确认前绝不执行；破坏性命令继续保留现有高风险二次确认。

## Module responsibilities / 模块职责

- `plan_cmd`: define and strictly parse workflow plans.
- `workflow_cmd`: validate step relationships, run pre-confirmation checks, skip unnecessary steps, sequence execution, and determine workflow status.
- `confirm_cmd`: display the complete plan and collect one confirmation.
- `execute_cmd`: execute one step, stream and capture output, and handle cancellation.
- `result_cmd`: build step records and the final workflow session record.
- `repair_cmd`: build a revised workflow plan from the failed step and previous results.
- `preflight_cmd`: continue to provide reusable read-only environment checks.

- `plan_cmd`：定义并严格解析 workflow 计划。
- `workflow_cmd`：校验步骤关系、运行确认前检查、跳过不必要步骤、顺序执行并判断任务状态。
- `confirm_cmd`：展示完整计划并收集一次确认。
- `execute_cmd`：执行单个步骤、实时输出并捕获结果、处理取消。
- `result_cmd`：生成步骤记录和最终 workflow session 记录。
- `repair_cmd`：根据失败步骤和已有结果生成修订计划。
- `preflight_cmd`：继续提供可复用的只读环境检查。

## Failure and repair / 失败与修复

- A failed check follows its declared `on_failure` behavior: continue, stop, or request repair.
- A failed modification step immediately stops all later modification steps.
- Modification steps are never retried automatically.
- A failed verification means the workflow failed even if all action commands exited with code 0.
- A revised plan includes the original request, previous plan, completed results, failed step, stdout, and stderr.
- Every revised plan is strictly validated and risk-classified again.
- A revised plan is displayed and confirmed again before any change.
- At most two repair cycles are allowed.

- 检查失败时按照声明的 `on_failure` 行为继续、停止或进入修复。
- 修改步骤失败后立即停止后续所有修改步骤。
- 修改步骤绝不自动重试。
- 即使 action 命令都以 0 退出，只要验证失败，workflow 就算失败。
- 修订计划的输入包含原始请求、旧计划、已完成结果、失败步骤、stdout 和 stderr。
- 每份修订计划都重新进行严格校验和风险分析。
- 修订计划在执行任何修改前必须重新展示并确认。
- 最多允许两轮修复。

Ctrl-C stops the current process and all later steps. Existing stdout and stderr are preserved. The workflow is recorded as `cancelled`.

按 Ctrl-C 会停止当前进程和所有后续步骤，保留已有 stdout 和 stderr，并把 workflow 记录为 `cancelled`。

## Session record / Session 记录

Each workflow stores:

- the original request;
- the confirmed plan;
- each step's command, risk, exit code, termination, stdout, and stderr;
- skipped steps and reasons;
- verification results;
- repair count;
- final status: `completed`, `failed`, or `cancelled`.

每个 workflow 保存：

- 原始用户请求；
- 已确认计划；
- 每一步的命令、风险、退出码、终止原因、stdout 和 stderr；
- 被跳过的步骤及原因；
- 验证结果；
- 修订次数；
- 最终状态：`completed`、`failed` 或 `cancelled`。

AI summary is optional and does not affect completion status.

AI summary 仍是可选功能，不参与任务完成判定。

## Testing and acceptance / 测试与验收

Automated tests cover strict parsing, invalid plans, duplicate IDs, risk escalation, read-only pre-checks, confirmation boundaries, skipped steps, step ordering, failure stops, no automatic modification retries, verification failure, two-repair limit, cancellation, and complete session records.

自动化测试覆盖严格解析、非法计划、重复 ID、风险提升、只读前置检查、确认边界、步骤跳过、执行顺序、失败停止、修改步骤不重试、验证失败、两次修复上限、取消和完整 session 记录。

Safe smoke tests cover:

- a read-only workflow that checks Rust and Git versions;
- a temporary-directory workflow that creates and verifies a file;
- a successful action followed by a failed verification;
- cancellation of a long-running step with confirmation that later steps did not run.

安全冒烟测试覆盖：

- 检查 Rust 和 Git 版本的纯只读 workflow；
- 在临时目录创建并验证文件；
- action 成功但最终验证失败；
- 取消长时间步骤，并确认后续步骤没有执行。

Required gates:

```text
cargo fmt --check
cargo test
cargo clippy --all --all-targets -- -D warnings
cargo build --release
git diff --check
```

The installed binary must also pass `aicmd help`, `aicmd doctor`, and interactive workflow smoke tests.

安装后的二进制还必须通过 `aicmd help`、`aicmd doctor` 和 workflow 交互冒烟测试。
