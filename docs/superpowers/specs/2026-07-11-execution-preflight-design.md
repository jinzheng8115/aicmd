# AICmd Execution Preflight Design
# AICmd 执行前检查设计

## Goal / 目标

Add a read-only preflight stage before command confirmation. AICmd uses checks
declared in the existing structured execution plan, validates and runs them
locally, and stops safely when the environment cannot satisfy the task.

在命令确认前增加只读的执行前检查阶段。AICmd 使用现有结构化执行计划中声明的检查项，
在本地校验并执行检查；当当前环境无法满足任务要求时安全停止。

## Scope / 范围

- Apply preflight checks to all executable `direct` and `script` plans.
- Keep one model request per task.
- Run checks locally without changing files, installing software, or elevating
  privileges.
- Stop execution when any required check fails.
- Show all failures and practical suggestions together.
- Record failed preflight results in the active session.
- Use the configured terminal language, with Chinese as the default.

- 对所有可执行的 `direct` 和 `script` 计划应用执行前检查。
- 每个任务仍然只调用一次模型。
- 所有检查在本地只读执行，不修改文件、不安装软件、不提升权限。
- 任一必要检查失败时停止执行。
- 一次显示全部失败原因和实际建议。
- 将失败的检查结果记录到当前 session。
- 遵循终端语言配置，默认使用中文。

## Non-goals / 非目标

- No automatic repair or dependency installation.
- No network connectivity probe.
- No sudo password or privilege-availability probe.
- No command-string heuristics that infer checks after planning.
- No generic policy engine or user-defined check scripts.
- No preflight stage for MCP search itself; a later `do` execution is checked.

- 不自动修复或安装依赖。
- 不探测网络连通性。
- 不测试 sudo 密码或权限可用性。
- 不在计划生成后通过命令字符串启发式推断检查项。
- 不增加通用策略引擎或用户自定义检查脚本。
- MCP 搜索本身不执行终端检查；后续进入 `do` 执行时再检查。

## Execution flow / 执行流程

```text
User task / 用户任务
  -> structured plan with preflight / 包含 preflight 的结构化计划
  -> strict JSON validation / 严格 JSON 校验
  -> local read-only checks / 本地只读检查
     -> pass / 通过
        -> risk display / 风险展示
        -> user confirmation / 用户确认
        -> execution / 执行
     -> fail / 失败
        -> show all failures and suggestions / 显示全部失败与建议
        -> record session note / 记录 session
        -> stop / 停止
```

Rules:

1. `preflight` is required for executable plans and may be an empty array.
2. A failed or errored check never falls through to command confirmation.
3. Users cannot bypass a failed required check.
4. `--dry-run` prints the complete structured plan and does not run checks.
5. `--print` prints only the generated command and does not run checks.
6. Explicit `do` and `err` execution paths use the same preflight runner.
7. MCP `search` does not run terminal preflight checks. If the user chooses
   `do`, the generated executable plan is checked before confirmation.

规则：

1. 可执行计划必须包含 `preflight`，但允许为空数组。
2. 检查失败或检查过程报错时，不进入命令确认。
3. 用户不能跳过失败的必要检查。
4. `--dry-run` 输出完整结构化计划，不执行检查。
5. `--print` 只输出生成的命令，不执行检查。
6. 显式 `do` 和 `err` 执行入口使用同一个检查器。
7. MCP `search` 不执行终端检查；用户选择 `do` 后，生成的可执行计划在确认前接受检查。

## Structured plan contract / 结构化计划协议

Example:

```json
{
  "mode": "script",
  "command": "python3 .aicmd/task.py",
  "preflight": [
    {
      "type": "command_exists",
      "value": "python3",
      "failure_message": "未找到 Python 3",
      "suggestion": "请先安装 Python 3"
    },
    {
      "type": "path_exists",
      "value": "data/orders.csv",
      "failure_message": "输入文件不存在",
      "suggestion": "请确认文件路径"
    }
  ]
}
```

Each check allows only:

- `type`
- `value`
- `failure_message`
- `suggestion`

Unknown fields and unknown check types invalidate the entire execution plan.
All four fields are required and must be non-empty. Display strings are never
executed or interpolated into shell commands.

每项检查只允许以下字段：

- `type`
- `value`
- `failure_message`
- `suggestion`

未知字段或未知检查类型会使整个执行计划无效。四个字段均为必填且不能为空。展示文本不会
被执行，也不会插入 Shell 命令。

## Supported checks / 支持的检查类型

| Type / 类型 | Behavior / 行为 |
| --- | --- |
| `command_exists` | Check whether an executable is available in `PATH`. / 检查可执行命令是否存在于 `PATH`。 |
| `path_exists` | Check whether a file or directory exists. / 检查文件或目录是否存在。 |
| `path_writable` | Check whether the target or its nearest existing parent is writable. / 检查目标或最近存在的父目录是否可写。 |
| `env_exists` | Check only whether an environment-variable name exists; never display its value. / 只检查环境变量名是否存在，绝不显示变量值。 |
| `os` | Require `macos` or `linux`. / 要求当前系统为 `macos` 或 `linux`。 |
| `git_clean` | Require the current directory to be inside a Git repository with no uncommitted changes. / 要求当前目录位于 Git 仓库中且没有未提交改动。 |

Path rules:

- Relative paths resolve from the current working directory.
- Absolute paths are accepted.
- Leading `~` expands to the current user's home directory.
- Shell expansion, command substitution, and variable expansion are forbidden.

路径规则：

- 相对路径从当前工作目录解析。
- 允许绝对路径。
- 开头的 `~` 展开为当前用户目录。
- 禁止 Shell 展开、命令替换和变量替换。

For `os`, `value` must be `macos` or `linux`. For `git_clean`, `value` is a
path to the repository working directory, normally `"."`.

`os` 的 `value` 只能是 `macos` 或 `linux`。`git_clean` 的 `value` 是 Git 仓库工作
目录，通常为 `"."`。

## Terminal behavior / 终端行为

When all checks pass, keep output compact:

```text
执行前检查：通过（3 项）
```

English:

```text
Preflight: passed (3 checks)
```

When checks fail:

```text
执行前检查失败

✗ 未找到 Python 3
  检查：command_exists = python3
  建议：请先安装 Python 3

✗ 输入文件不存在
  检查：path_exists = data/orders.csv
  建议：请确认文件路径

命令未执行。
```

English output uses the same structure without showing Chinese text. The
configured `language: zh|en` controls terminal labels. Model-provided
`failure_message` and `suggestion` should use the configured language.

英文输出采用相同结构且不同时显示中文。终端标签由 `language: zh|en` 控制。模型生成的
`failure_message` 和 `suggestion` 应使用当前配置语言。

## Code boundaries / 代码边界

### `src/preflight_cmd.rs`

- Define `PreflightCheck`, supported check types, and result types.
- Validate check values before execution.
- Execute local read-only checks.
- Return all failures instead of stopping at the first failure.
- Format compact success and detailed failure output.

### `src/plan_cmd.rs`

- Add `preflight` to executable execution plans.
- Preserve strict deserialization and unknown-field rejection.
- Require `preflight` for `direct` and `script`.
- Reject preflight checks on invalid plan shapes.

### `src/main.rs`

- Invoke the shared preflight runner after plan validation and before command
  confirmation.
- Stop when any check fails.
- Keep `--dry-run` and `--print` free of check execution.

### `assets/roles/%shell%.md`

- Require the planner to emit necessary preflight checks.
- Allow an empty array for simple dependency-free read-only commands.
- Never ask the planner to repair a failed environment automatically.
- Generate display messages in the configured terminal language.

### `src/result_cmd.rs`

- Build a session note for failed preflight checks.
- Do not record a successful command-cache entry when execution never started.

## Error handling / 错误处理

- Invalid plan schema: return the existing invalid-plan error and execute
  nothing.
- Invalid check value: treat the plan as invalid, not as a runtime warning.
- Failed check: show every failed item, record the result, and stop.
- Check implementation error: report it as a failed check and stop.
- Missing Git executable for `git_clean`: fail the check with its configured
  message and suggestion.
- Secret environment values are never read into output or session data.

- 计划结构无效：返回现有无效计划错误，不执行任何命令。
- 检查参数无效：视为计划无效，不作为运行时警告忽略。
- 检查失败：显示全部失败项、记录结果并停止。
- 检查实现报错：作为检查失败处理并停止。
- `git_clean` 缺少 Git 命令：使用配置的失败信息和建议停止。
- 敏感环境变量值不会进入输出或 session 数据。

## Verification / 验证

Unit tests:

1. One passing and one failing case for each of the six check types.
2. Unknown check type, unknown field, missing field, and empty field rejection.
3. Multiple failures are returned together and in input order.
4. Environment-variable values never appear in results.
5. Relative, absolute, and home-relative path handling.
6. Chinese and English terminal labels.
7. Failed preflight does not reach command execution.
8. `--dry-run` and `--print` do not run checks.

Manual verification:

```bash
aicmd 当前目录有多少文件
aicmd 读取 missing.csv 并生成 output.csv
aicmd 在当前 Git 项目批量修改配置文件
aicmd --dry-run 读取 missing.csv 并生成 output.csv
aicmd --print 当前目录有多少文件
```

Project checks:

```bash
cargo fmt --check
cargo test
cargo clippy --all --all-targets -- -D warnings
cargo build --release
git diff --check
```
