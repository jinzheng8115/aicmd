# Context Fix Report / 上下文修复报告

## Result / 结果

Fixed the P1 continuation-context defect from base HEAD `1eda511`.

已修复基线 HEAD `1eda511` 上的 P1 继续任务上下文缺失问题。

- `Input::with_role` now changes only the role and preserves the input's existing session-context state.
- `Session::build_messages` now builds enabled context as:
  1. current input role system prompt and examples;
  2. up to eight recent non-system session messages;
  3. current user input.
- Old session system prompts are not reused.
- Context-disabled and no-context-marker requests keep the previous role-only behavior.
- No dependency was added.

- `Input::with_role` 现在只切换角色，并保留 input 原有的 session context 状态。
- `Session::build_messages` 在启用上下文时按以下顺序构造消息：
  1. 当前 input role 的 system prompt 和 examples；
  2. 最近最多八条非 system 会话消息；
  3. 当前 user 输入。
- 不再复用旧 session system prompt。
- context disabled 或没有上下文 marker 时保持原来的 role-only 行为。
- 未增加依赖。

## Root cause / 根因

Planner and command-generation paths both switch roles through `Input::with_role`. That method set `with_session = false`, so the planner could not see the saved failure execution note. Even if context was enabled, `Session::build_messages` retained the old session system prompt instead of rebuilding the request from the current role.

planner 和 command-generation 路径都会通过 `Input::with_role` 切换角色。该方法原来把 `with_session` 设为 `false`，导致 planner 看不到保存的失败执行记录。即使上下文已启用，`Session::build_messages` 也会保留旧 session system prompt，而不是使用当前 role 重建请求。

## Tests / 测试

Added regression coverage proving:

- role switching preserves session context;
- current-role system prompt and examples replace the old session system prompt;
- recent `Command` and `Exit code` execution-note fields enter planner messages;
- planner and command-generation built-in roles each use their own current prompt;
- context-disabled and no-marker requests keep prior behavior.

新增回归测试证明：

- role 切换后 session context 仍然保留；
- 当前 role 的 system prompt 和 examples 替代旧 session system prompt；
- 最近 execution note 中的 `Command` 和 `Exit code` 会进入 planner messages；
- planner 和 command-generation 内置角色分别使用自己的当前 prompt；
- context disabled 和无 marker 请求保持原行为。

The existing main test `continue_last_failure_uses_daily_session_and_normal_planner_text` still proves that the deterministic continuation intent sets `no_cache = true`.

现有 main 测试 `continue_last_failure_uses_daily_session_and_normal_planner_text` 继续证明固定继续表达会设置 `no_cache = true`。

## Local fake-model verification / 本地 fake model 验证

Used a temporary config, daily session, and local OpenAI-compatible HTTP server. The command was run with `--print`, so it sent a real planner request to the fake server but did not execute the returned command.

使用临时配置、每日 session 和本地 OpenAI-compatible HTTP server。命令通过 `--print` 运行，因此会向 fake server 发送真实 planner 请求，但不会执行返回的命令。

Observed:

```text
fake-model continuation request: PASS
messages: 3
contains Command/Exit code: yes
old session system prompt reused: no
printed command: printf recovered
```

## Documentation / 文档

Updated:

- `README.md`
- `README.en.md`
- `docs/aicmd-usage.md`

The docs now state that continuation requests can reference saved failure records, retain cache bypass, use recent non-system history, and keep the current planner or command-generation system prompt.

文档现在明确：继续请求可以引用已保存的失败记录，仍然绕过缓存，使用最近的非 system 历史，并保留当前 planner 或 command-generation 角色自己的 system prompt。

## Karpathy review / Karpathy 审查

- The production fix changes only the two shared root-cause points.
- No planner/main abstraction or dependency was added.
- Tests cover the requested behavior without changing unrelated flows.
- Documentation describes verified behavior rather than future or automatic-repair claims.

- 生产代码只修改两个共享根因位置。
- 没有增加 planner/main 抽象或新依赖。
- 测试覆盖要求行为，没有改变无关流程。
- 文档只描述已验证行为，没有未来承诺或自动修复承诺。

## Full verification / 完整验证

```text
cargo fmt --all
exit 0

cargo fmt --all -- --check
exit 0

cargo test
111 passed; 0 failed

cargo clippy --all --all-targets -- -D warnings
exit 0

cargo build --release
exit 0

target/release/aicmd --version
aicmd 0.4.2

git diff --check
exit 0
```
