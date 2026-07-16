# Task 3 Report / Task 3 报告

## Status / 状态

Task 3 is implemented without new dependencies. AICmd now captures Git porcelain state only for commands already classified as `ChangesSystem` or `Destructive`, compares the state after command completion, and prints only new records or records whose status changed.

Task 3 已完成，未新增依赖。AICmd 现在只对现有风险分类为 `ChangesSystem` 或 `Destructive` 的命令捕获 Git porcelain 状态，在命令结束后比较状态，并且只报告新增记录或状态发生变化的记录。

Git capture failures, invalid output, and non-Git directories return no snapshot and do not block command execution. Non-zero command exits still reach the post-execution capture and report path.

Git 捕获失败、输出无效以及非 Git 目录都会返回空快照，不阻断命令执行。命令以非零状态退出时，仍会执行命令后的捕获与报告流程。

## Implementation / 实现

- Added `GitSnapshot::capture` using `git status --porcelain=v1 --untracked-files=all`.
- Store complete porcelain records keyed by their displayed path.
- Report only records absent before execution or changed for the same displayed path.
- Recommend `git diff` and manual recovery.
- Do not run `git reset`, delete files, or perform automatic recovery.
- Keep read-only commands outside the snapshot path.

- 新增 `GitSnapshot::capture`，执行 `git status --porcelain=v1 --untracked-files=all`。
- 以显示路径为键保存完整 porcelain 记录。
- 只报告执行前不存在的记录，或同一路径状态发生变化的记录。
- 提示使用 `git diff` 并手动恢复。
- 不执行 `git reset`、删除文件或任何自动恢复。
- 只读命令不会进入快照流程。

## TDD Evidence / TDD 证据

Delta and report tests failed first because the new API did not exist:

delta 与报告测试先失败，因为新 API 尚不存在：

```text
error[E0433]: failed to resolve: use of undeclared type `GitSnapshot`
error[E0425]: cannot find function `format_recovery_report` in this scope
```

The risk gate test also failed before its implementation:

风险门控测试同样在实现前失败：

```text
error[E0599]: no method named `captures_git_changes` found for enum `CommandRiskLevel`
```

After the minimal implementation:

完成最小实现后：

```text
cargo test change_report_cmd::tests
4 passed; 0 failed

cargo test git_change_capture_is_limited_to_modifying_risk_levels
1 passed; 0 failed
```

## Verification / 验证

```text
cargo fmt --all -- --check
exit 0

cargo test change_report_cmd::tests
4 passed; 0 failed

cargo test
87 passed; 0 failed

cargo clippy --all-targets --all-features -- -D warnings
exit 0

git diff --check
exit 0
```

## Changed Files / 修改文件

- `src/change_report_cmd.rs`
- `src/main.rs`
- `.superpowers/sdd/task-3-report.md`

## Concerns / 注意事项

- No blocking concerns.
- Snapshot capture is intentionally advisory and silent on failure.
- The tests cover delta calculation, report guidance, and risk gating. The interactive confirmation/execution path is not driven through a subprocess TTY test in this task.

- 无阻塞问题。
- 快照捕获按设计仅提供建议，失败时保持静默。
- 测试覆盖差量计算、恢复提示和风险门控；本任务未通过子进程 TTY 测试驱动完整交互确认与执行路径。
