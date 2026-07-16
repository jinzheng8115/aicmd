# Task 4 Final Report / Task 4 最终报告

## Status / 状态

All final-review findings are fixed without adding commands or dependencies.

最终审查问题已全部修复，未新增命令或依赖。

## Final Review Fixes / 最终审查修复

1. **MCP child lifecycle**
   - A single `McpChildGuard` now owns every successfully spawned MCP child.
   - Every return path, including success and errors from pipe setup, initialize, tool discovery, selection, calls, or response handling, runs `kill` and `wait`.
   - Cleanup errors are ignored so the original runtime error remains unchanged in the `anyhow` source chain.

1. **MCP 子进程生命周期**
   - 使用统一的 `McpChildGuard` 接管每个成功启动的 MCP 子进程。
   - 成功返回以及管道、初始化、工具发现、工具选择、工具调用、响应处理的错误返回都会执行 `kill` 和 `wait`。
   - 清理错误不会覆盖原始错误，原错误仍保留在 `anyhow` source chain 中。

2. **Shared doctor/runtime validation**
   - Doctor and runtime now reuse the same validation helpers for server `command`, command `server`, and optional `tool`.
   - Empty strings, non-string values, and leading/trailing whitespace are rejected instead of silently trimming values.
   - Missing server `type` defaults to `stdio`; a present non-string `type` is rejected in both paths.

2. **Doctor 与 runtime 共用校验**
   - Doctor 与 runtime 复用同一组 helper 校验 server `command`、command `server` 和可选 `tool`。
   - 空字符串、非字符串以及首尾空白都会报错，不再 trim 后误判为有效。
   - server `type` 缺失时默认 `stdio`；字段存在但不是字符串时，两个路径都会报错。

3. **Search doctor compatibility**
   - Restored the legacy `Search` check states: `configured`, `command not configured`, and `not checked` when the MCP config is missing, unreadable, or invalid JSON.

3. **Search doctor 兼容性**
   - 恢复旧版 `Search` 检查状态：`configured`、`command not configured`，以及 MCP 配置缺失、不可读或 JSON 无效时的 `not checked`。

4. **Structured timeout guidance**
   - Local `recv_timeout` failures use the typed `McpResponseTimeout` error.
   - Only that structured error suggests `AICMD_MCP_START_TIMEOUT_SECS` or `AICMD_MCP_CALL_TIMEOUT_SECS`.
   - RPC error text containing `timed out` still recommends `aicmd doctor`.

4. **结构化 timeout 建议**
   - 本地 `recv_timeout` 失败使用类型化的 `McpResponseTimeout` 错误。
   - 只有该结构化错误会建议调整 `AICMD_MCP_START_TIMEOUT_SECS` 或 `AICMD_MCP_CALL_TIMEOUT_SECS`。
   - RPC 错误文本即使包含 `timed out`，仍建议运行 `aicmd doctor`。

5. **Fake MCP lifecycle regression**
   - The test binary acts as a minimal fake stdio MCP server.
   - It covers success, initialize error, and RPC call error while deliberately remaining alive after its response.
   - The parent verifies the child PID no longer exists after `call_mcp_with_config` returns, catching both live-process and zombie leaks.
   - Sentinel environment secrets are not present in diagnostics or runtime error output.

5. **Fake MCP 生命周期回归测试**
   - 测试二进制自身作为最小 fake stdio MCP server。
   - 覆盖成功、initialize error 和 RPC call error，并在响应后故意保持存活。
   - 父测试在 `call_mcp_with_config` 返回后确认 PID 已不存在，可捕获存活进程和 zombie 泄漏。
   - diagnostic 与 runtime 错误输出均不包含 sentinel 环境 secret。

## TDD Evidence / TDD 证据

The new regression tests failed before implementation because the shared runtime helper, structured timeout type, and Search diagnostic helper did not exist:

新增回归测试在实现前失败，因为共用 runtime helper、结构化 timeout 类型和 Search diagnostic helper 尚不存在：

```text
error[E0422]: cannot find struct, variant or union type `McpResponseTimeout`
error[E0425]: cannot find function `call_mcp_with_config`
error[E0425]: cannot find function `search_diagnostic`
```

Focused verification after the fixes:

修复后的 focused 验证：

```text
cargo test mcp_cmd::tests
16 passed; 0 failed

cargo test doctor_cmd::tests
2 passed; 0 failed
```

## Final Verification / 最终验证

```text
cargo test
106 passed; 0 failed

cargo clippy --all-targets -- -D warnings
exit 0

cargo fmt --check
exit 0

git diff --check
exit 0
```

The shell did not expose Cargo directly, so verification prepended `$HOME/.cargo/bin` to `PATH`.

当前 shell 未直接暴露 Cargo，因此验证时将 `$HOME/.cargo/bin` 加入 `PATH` 前部。

## Changed Files / 修改文件

- `src/mcp_cmd.rs`
- `src/doctor_cmd.rs`
- `.superpowers/sdd/task-4-report.md`

## Scope / 范围

- No new CLI commands.
- No new dependencies.
- The child PID existence assertion is Unix-only; the lifecycle guard itself remains platform-independent.

- 未新增 CLI 命令。
- 未新增依赖。
- 子进程 PID 存在性断言仅在 Unix 运行；生命周期 guard 本身保持跨平台。
