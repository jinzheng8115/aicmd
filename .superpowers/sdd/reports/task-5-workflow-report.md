# Task 5 Workflow Repair Report / Task 5 工作流修复报告

## Outcome / 结果

Implemented Task 5 on branch `spec/workflow-execution` from base `1105d8454a1be04fc5a50d0ec345c2de01029d3d`. No unresolved architecture issue remains, and `src/result_cmd.rs` did not require changes.

已在分支 `spec/workflow-execution`、基线 `1105d8454a1be04fc5a50d0ec345c2de01029d3d` 上完成 Task 5。目前没有未解决的架构问题，也无需修改 `src/result_cmd.rs`。

## Implementation / 实现内容

- Added `WorkflowRepairContext` and a bilingual repair prompt containing the original request, complete previous strict workflow JSON, prior results, failed step, exit code, stdout, and stderr.
- The prompt requires one strict JSON `workflow` plan, safe preservation of completed outcomes, a read-only verification step, no Markdown/prose/single-command response, and renewed whole-plan approval.
- Added `MAX_WORKFLOW_REPAIRS = 2` and `can_repair_workflow`.
- A failed `Repair` step now creates a planner `Input` with current session context and calls existing `request_execution_plan` with a fresh request-level `RetryBudget`.
- Non-workflow model responses are rejected after strict parsing.
- Every repaired plan starts again from local check-risk preparation and is displayed and confirmed as a whole plan, including read-only-only revised plans.
- Each recursive execution increments `repair_attempts`; attempts `0` and `1` may repair, while attempt `2` is final.
- Existing controlled command execution remains unchanged: cancellation preserves partial output and exit `130`; failed modification commands receive one `StepResult` and are never retried.
- Each terminal plan record is saved before repair generation. The final revised-plan record therefore carries its actual repair count.

- 新增 `WorkflowRepairContext` 与双语修复提示词，包含原始请求、完整旧版严格 workflow JSON、已有结果、失败步骤、退出码、stdout 和 stderr。
- 提示词要求只返回一个严格 JSON `workflow` 计划，安全保留已完成结果，包含只读验证步骤，不允许 Markdown、说明文字或单条修复命令，并要求重新批准完整计划。
- 新增 `MAX_WORKFLOW_REPAIRS = 2` 与 `can_repair_workflow`。
- `Repair` 步骤失败后，会创建带当前 Session 上下文的 planner `Input`，并用新的请求级 `RetryBudget` 调用现有 `request_execution_plan`。
- 模型输出先经过严格解析，非 workflow 模式会被拒绝。
- 每份修订计划都重新执行本地 check 风险准备，并重新展示、确认整个计划；即使修订计划只有只读步骤，也必须重新确认。
- 每次递归执行都会增加 `repair_attempts`；次数为 `0`、`1` 时允许修复，到 `2` 时停止。
- 现有受控命令执行语义保持不变：取消会保留部分输出和退出码 `130`；失败修改命令只产生一条 `StepResult`，不会重试。
- 每个计划的终止记录都会在生成修订计划前保存；最终修订计划记录包含真实修复次数。

## TDD Evidence / TDD 证据

### Prescribed prompt and limit RED / 提示词与上限 RED

The brief's exact two-filter command is not accepted by Cargo:

```text
cargo test repair_cmd::tests workflow_cmd::tests::repair_limit_is_two
exit 1
error: unexpected argument 'workflow_cmd::tests::repair_limit_is_two' found
```

Cargo only accepts one positional test filter. The Cargo-compatible RED command was:

```text
cargo test repair_cmd::tests -- --nocapture
exit 101
```

Compilation failed for the expected missing APIs: `WorkflowRepairContext`, `build_workflow_repair_prompt`, and `can_repair_workflow`.

任务简报中的双过滤器命令不被 Cargo 接受；改用单过滤器命令后，按预期因上述接口不存在而以 `101` 编译失败。

### Integration RED / 集成 RED

```text
cargo test tests::repair_prompt_uses_aggregate_failed_workflow_record -- --nocapture
exit 101
```

Initially failed because `build_next_workflow_repair_prompt` and `require_workflow_repair_plan` did not exist. A later strict-old-plan assertion also failed with exit `101` until the prompt included `"mode": "workflow"`.

最初因聚合记录到修复提示词、修订模式校验接口不存在而失败；随后新增的旧计划严格模式断言也先以 `101` 失败，直到提示词包含 `"mode": "workflow"`。

```text
cargo test workflow_cmd::tests::repaired_workflow_requires_confirmation_even_when_read_only -- --nocapture
exit 101
```

Failed because repaired-plan confirmation enforcement did not exist.

该测试因尚未实现修订计划强制重新确认而失败。

### GREEN / GREEN 结果

```text
cargo test repair_cmd::tests
3 passed; 0 failed

cargo test workflow_cmd::tests
22 passed; 0 failed

cargo test tests::repair_prompt_uses_aggregate_failed_workflow_record
1 passed; 0 failed

cargo test tests::repaired_plan_must_remain_workflow_mode
1 passed; 0 failed
```

The workflow suite includes real controlled-process tests proving partial-output cancellation and one-attempt modification failure.

workflow 测试集包含真实受控进程测试，验证取消时保留部分输出，以及失败修改命令只执行一次。

## Final Gate / 最终门禁

```text
cargo fmt --check
exit 0

cargo test
176 passed; 0 failed

cargo clippy --all --all-targets -- -D warnings
exit 0

git diff --check
exit 0
```

## Scope and Review / 范围与审查

Production changes are limited to the owned files:

生产改动仅限任务归属文件：

- `src/repair_cmd.rs`
- `src/workflow_cmd.rs`
- `src/main.rs`

No dependency, configuration, command retry, cache reuse, background execution, parallel workflow execution, or generic agent loop was added. Existing untracked SDD files were not modified or staged, except this required report.

未增加依赖、配置、命令重试、缓存复用、后台执行、并行 workflow 执行或通用 Agent 循环。除本任务要求的报告外，没有修改或暂存已有未跟踪 SDD 文件。

Karpathy-guidelines review: the implementation reuses Task 4's parser, retry budget, record builder, confirmation, controlled executor, and cancellation flow; adds only the repair-specific prompt, limit, confirmation override, and recursive orchestration required by the brief.

Karpathy-guidelines 审查结论：实现复用了 Task 4 的解析器、重试预算、记录构造、确认、受控执行和取消流程，只新增任务要求的修复提示词、次数上限、修订计划确认覆盖和递归编排。

---

## Review Findings Remediation / 审查问题修复

Review baseline / 审查基线：`1e5a6c3 feat: repair failed workflows safely`

### Fixes / 修复内容

1. **Effective read-only safety / 本地有效只读安全**
   - Added one shared `validate_read_only_workflow_steps` check for every `Check` and `Verify` step.
   - `run_workflow_plan` calls it before the first check; `execute_prepared_workflow` also calls it before any pending action for direct-helper safety.
   - A modifying verify command mislabeled `read_only` is rejected before any command runs. The existing terminal aggregation path saves the failure, and no repair failure exists to trigger planning.
   - 新增共享的 `validate_read_only_workflow_steps`，检查每个 `Check` 与 `Verify` 的本地有效风险。
   - `run_workflow_plan` 在首个 check 前调用；`execute_prepared_workflow` 也在任何待执行 action 前调用，保证直接调用 helper 时同样安全。
   - 被错误标记为 `read_only` 的修改型 verify 会在任何命令执行前被拒绝；失败通过现有聚合记录路径保存，且不会触发修复规划。

2. **Cancellation gates / 取消门禁**
   - Added pure `decide_workflow_repair_transition` states: `Stop`, `RequestPlanner`, and `ExecuteRevisedPlan`.
   - The shared abort signal is checked immediately before planner invocation and again after strict parsing before recursive execution.
   - Pre-planner, in-planner, and post-planner cancellation paths save cancelled semantics and exit `130`; none enters revised execution.
   - 新增纯函数 `decide_workflow_repair_transition`，状态为 `Stop`、`RequestPlanner`、`ExecuteRevisedPlan`。
   - 在调用 planner 前立即检查共享 abort signal，并在严格解析后、递归执行前再次检查。
   - planner 前、planner 中和 planner 后的取消路径都会保存 cancelled 语义并以 `130` 退出，不会进入修订计划执行。

3. **Prompt injection boundary / Prompt 注入边界**
   - Added concise bilingual instructions declaring original request, previous plan/results, stdout, and stderr as untrusted data, never instructions.
   - All context fields are JSON serialized between stable `UNTRUSTED_WORKFLOW_DATA_BEGIN` and `UNTRUSTED_WORKFLOW_DATA_END` markers. Embedded newlines and delimiter text remain JSON string data.
   - 新增简洁双语规则，明确原始请求、旧计划/结果、stdout、stderr 都是不可信数据，不能作为指令。
   - 全部上下文字段经过 JSON 序列化，并放在稳定的 `UNTRUSTED_WORKFLOW_DATA_BEGIN` / `UNTRUSTED_WORKFLOW_DATA_END` 标记之间；嵌入的换行和分隔符文本仍是 JSON 字符串数据。

4. **Bounded integration evidence / 有界集成证据**
   - Pure transition tests prove attempts `0` and `1` may request planning, attempt `2` stops, and both pre/post-planner abort states stop.
   - The same state test creates aggregate records and verifies `repair_attempts` remains exactly `0`, `1`, and `2`.
   - Existing real-shell regression still proves a failed modifying command runs once and receives one `StepResult`; no command retry or generic loop was added.
   - 纯状态测试证明次数 `0`、`1` 可进入规划，次数 `2` 停止，planner 前后 abort 都会停止。
   - 同一状态测试创建聚合记录并验证 `repair_attempts` 精确保持为 `0`、`1`、`2`。
   - 现有真实 shell 回归测试继续证明失败修改命令只执行一次且只有一条 `StepResult`；未增加命令重试或通用循环。

### Review RED Evidence / 审查 RED 证据

```text
cargo test workflow_cmd::tests::modifying_verify_declared_read_only_is_rejected_before_execution -- --nocapture
exit 101
error[E0425]: cannot find function `validate_read_only_workflow_steps`

cargo test tests::repair_transition_stops_on_pre_and_post_planner_cancellation -- --nocapture
exit 101
error[E0425]: cannot find function `decide_workflow_repair_transition`
error[E0433]: use of undeclared type `WorkflowRepairTransition`

cargo test workflow_cmd::tests::unsafe_verify_is_rejected_before_prior_action_runs -- --nocapture
exit 101
called `Result::unwrap_err()` on an `Ok` value: Completed
```

The prompt-boundary assertions were added before implementation in the same RED compile step; compilation also failed on the missing safety and transition APIs. After those APIs compiled, the new bilingual untrusted-data and stable-delimiter assertions passed.

Prompt 边界断言与其他 RED 测试同时先写；该轮编译因缺少安全与 transition API 而失败。相关 API 可编译后，新增的双语不可信数据规则与稳定分隔符断言通过。

### Review GREEN Evidence / 审查 GREEN 证据

```text
cargo test repair_cmd::tests
4 passed; 0 failed

cargo test workflow_cmd::tests
25 passed; 0 failed

cargo test tests::repair_prompt_uses_aggregate_failed_workflow_record
1 passed; 0 failed

cargo test tests::repair_transition_stops_on_pre_and_post_planner_cancellation
1 passed; 0 failed

cargo test tests::repair_transition_and_records_are_bounded_to_two_attempts
1 passed; 0 failed

cargo test tests::unsafe_verify_is_saved_as_failure_without_repair_transition
1 passed; 0 failed
```

### Review Final Gate / 审查最终门禁

```text
cargo fmt --check
exit 0

cargo test
183 passed; 0 failed

cargo clippy --all --all-targets -- -D warnings
exit 0

git diff --check
exit 0
```

No warnings, test failures, formatting errors, or diff whitespace errors remained. Changes are limited to `src/main.rs`, `src/repair_cmd.rs`, `src/workflow_cmd.rs`, and this report.

最终无 warning、测试失败、格式错误或 diff 空白错误。改动仅限 `src/main.rs`、`src/repair_cmd.rs`、`src/workflow_cmd.rs` 和本报告。

### Review / 审查

Karpathy-guidelines review: the fix uses one shared risk validator and one pure repair-transition function, reuses the existing Task 4 aggregate/cancellation machinery, adds no dependency or configuration, and leaves command execution/retry behavior unchanged.

Karpathy-guidelines 审查：修复使用一个共享风险校验器和一个纯 repair transition 函数，复用 Task 4 的聚合记录与取消机制，不新增依赖或配置，并保持命令执行/重试行为不变。

Known concerns / 已知关注点：none.
