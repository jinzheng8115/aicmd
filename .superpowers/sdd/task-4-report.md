# Task 4 Report / Task 4 报告

## Status / 状态

Task 4 is implemented without new commands or dependencies. `aicmd doctor` now parses MCP JSON and validates configured servers, command mappings, `stdio` type, non-empty commands, executable availability, and optional tool names without starting a server or printing values from `env`.

Task 4 已完成，未新增命令或依赖。`aicmd doctor` 现在会解析 MCP JSON，并检查 server、command 映射、`stdio` 类型、非空 command、可执行文件可用性和可选 tool 名称；检查过程不会启动 server，也不会打印 `env` 中的值。

MCP runtime errors now identify `start`, `initialize`, `tools/list`, `tool selection`, or `tools/call`. Timeout errors point to the existing timeout environment variable; other errors recommend `aicmd doctor`. The original error remains in the `anyhow` source chain.

MCP 运行时错误现在会标明 `start`、`initialize`、`tools/list`、`tool selection` 或 `tools/call`。超时错误会提示现有超时环境变量，其他错误建议运行 `aicmd doctor`；原始错误保留在 `anyhow` source chain 中。

## Implementation / 实现

- Added `McpDiagnostic` and `diagnose_config()`.
- Reused `mcp_root` and `mcp_commands`; added pure `diagnose_value`.
- Path-containing commands use `Path::is_file`; bare commands scan `PATH`.
- Replaced Doctor's coarse MCP/Search checks with one check per diagnostic.
- Added `mcp_stage_error` and stage wrappers around MCP startup, initialization, discovery, selection, and call handling.
- Added focused unit tests for invalid JSON, missing mappings, unsupported type, empty command/tool, missing absolute/PATH executables, valid configuration, secret omission, stage labels, timeout guidance, and source preservation.

- 新增 `McpDiagnostic` 与 `diagnose_config()`。
- 复用 `mcp_root` 和 `mcp_commands`，新增纯函数 `diagnose_value`。
- 含路径的 command 使用 `Path::is_file`；裸命令扫描 `PATH`。
- Doctor 原有粗粒度 MCP/Search 检查改为每条 diagnostic 对应一条检查。
- 新增 `mcp_stage_error`，覆盖 MCP 启动、初始化、工具发现、工具选择和工具调用阶段。
- 新增 focused 单元测试，覆盖无效 JSON、缺失映射、不支持类型、空 command/tool、缺失绝对路径/PATH 可执行文件、有效配置、secret 不泄漏、阶段标签、超时建议和 source 保留。

## TDD Evidence / TDD 证据

The first focused run failed because the requested diagnostic and stage-error APIs did not exist:

首次 focused 测试因所需 diagnostic 和 stage-error API 尚不存在而失败：

```text
error[E0432]: unresolved import `crate::mcp_cmd::McpDiagnostic`
error[E0425]: cannot find function `diagnose_value` in this scope
error[E0425]: cannot find function `mcp_stage_error` in this scope
error[E0425]: cannot find function `mcp_checks` in this scope
```

The invalid-file test also failed before the file-loading helper was added:

无效文件测试同样在 file-loading helper 实现前失败：

```text
error[E0425]: cannot find function `diagnose_path` in this scope
```

After the minimal implementation:

完成最小实现后：

```text
cargo test mcp_cmd::tests
11 passed; 0 failed

cargo test doctor_cmd::tests
1 passed; 0 failed
```

## Verification / 验证

```text
cargo fmt --check
exit 0

cargo test
100 passed; 0 failed

cargo clippy --all-targets -- -D warnings
exit 0

cargo build
exit 0
```

A temporary MCP config with a missing executable and a sentinel secret was checked manually:

使用包含缺失可执行文件和 sentinel secret 的临时 MCP 配置进行了手动检查：

- Doctor reported `MCP server broken: error executable not found in PATH`.
- The sentinel secret did not appear in Doctor output.
- `aicmd mcp search test` failed at `MCP start`, recommended `aicmd doctor`, and retained the spawn/OS errors in the cause chain.

- Doctor 输出 `MCP server broken: error executable not found in PATH`。
- Doctor 输出中未出现 sentinel secret。
- `aicmd mcp search test` 在 `MCP start` 阶段失败，建议运行 `aicmd doctor`，并在 cause chain 中保留 spawn/OS 原始错误。

The shell environment did not include Cargo, so verification prepended `$HOME/.cargo/bin` to `PATH`.

当前 shell 环境未包含 Cargo，因此验证时将 `$HOME/.cargo/bin` 加入 `PATH` 前部。

## Changed Files / 修改文件

- `src/mcp_cmd.rs`
- `src/doctor_cmd.rs`
- `.superpowers/sdd/task-4-report.md`

## Concerns / 注意事项

- No blocking concerns.
- Doctor intentionally checks executable file presence only; it does not start servers, probe networks, or validate server arguments.
- A command mapping can be valid while its referenced server has a separate error; Doctor prints both diagnostics.

- 无阻塞问题。
- Doctor 有意只检查可执行文件是否存在，不启动 server、不探测网络，也不验证 server 参数。
- command 映射本身可以有效，而被引用的 server 另有错误；Doctor 会分别输出两条诊断。
