# Final Minor Documentation Fix Report / 最终轻微文档修复报告

## Scope / 范围

Updated only the structured workflow documentation in `assets/roles/%shell%.md` and `docs/aicmd-usage.md`.

仅更新 `assets/roles/%shell%.md` 和 `docs/aicmd-usage.md` 中的结构化 workflow 文档。

The bilingual rule now states that `run_if` is allowed only on `action` and `verify` steps, forbidden on `check` steps, and may reference only an earlier `check` with `passed` or `failed`.

双语规则现已明确：`run_if` 只能用于 `action` 和 `verify` 步骤，禁止用于 `check` 步骤，并且只能引用更早的 `check` 及其 `passed` 或 `failed` 结果。

No runtime code, unrelated documentation, or README files were changed.

未修改运行时代码、无关文档或 README。

## Verification / 验证

All requested checks passed:

所有要求的检查均已通过：

- `cargo test config::role::tests -- --nocapture` — 4 passed
- `cargo test plan_cmd::tests -- --nocapture` — 11 passed
- `cargo test config::tests -- --nocapture` — 1 passed
- `cargo fmt --check` — passed
- `cargo test` — 189 passed, 0 failed
- `git diff --check` — passed

Reviewed the documentation with `karpathy-guidelines`: the change is surgical, bilingual, and limited to the requested schema constraint.

已使用 `karpathy-guidelines` 审查文档：改动聚焦、双语且仅限用户要求的 schema 约束。
