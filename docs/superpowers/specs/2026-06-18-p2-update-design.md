# AICmd P2 Update Design

中文：本文定义 P2 UX 改进：新增 `aicmd update`，让用户不需要记住长安装命令即可更新 AICmd。

English: This document defines the P2 UX improvement: add `aicmd update` so users can update AICmd without remembering the long installer command.

## Goals / 目标

中文：
- 用户可以运行 `aicmd update` 更新到最新 Release。
- 用户可以运行 `aicmd update --version vX.Y.Z` 安装指定版本。
- 更新会二次确认，因为它会覆盖本地 AICmd 二进制。
- 更新完成后提示运行 `aicmd doctor`。

English:
- Users can run `aicmd update` to update to the latest Release.
- Users can run `aicmd update --version vX.Y.Z` to install a specific version.
- Update asks for confirmation because it overwrites the local AICmd binary.
- After update, AICmd suggests running `aicmd doctor`.

## Non-Goals / 非目标

中文：
- 本期不实现自动检查最新版本与当前版本差异。
- 本期不实现后台自更新或静默更新。
- 本期不改变 release 打包流程。
- 本期不修改 installer 的网络 fallback 行为。

English:
- This phase does not compare the installed version against the latest version.
- This phase does not implement background or silent updates.
- This phase does not change release packaging.
- This phase does not change installer network fallback behavior.

## Command / 命令

```text
aicmd update
aicmd update --version v0.30.1
aicmd update --dry-run
aicmd update --version v0.30.1 --dry-run
```

Behavior / 行为：
- On macOS/Linux, run the existing shell installer through `bash`.
- On Windows, run the existing PowerShell installer through PowerShell.
- `--version` passes the version to the installer.
- `--dry-run` prints the command that would run and exits without changing files.
- Without `--dry-run`, ask for confirmation before running the installer.
- Unknown options print compact help and return non-zero.

## Safety / 安全

中文：
- 确认提示必须说明会重新下载安装并覆盖本地 AICmd。
- `--dry-run` 必须不访问网络、不写文件。
- 安装器 stdout/stderr 保持直出，方便用户看到失败原因。

English:
- The confirmation prompt must say that AICmd will be downloaded and overwritten.
- `--dry-run` must not access the network or write files.
- Installer stdout/stderr should remain visible so users can diagnose failures.

## Testing / 测试

Manual checks / 手动检查：
- `aicmd update --dry-run`
- `aicmd update --version v0.30.1 --dry-run`
- `printf 'n\n' | aicmd update` cancels.
- Local source install after implementation still works.

Automated checks / 自动检查：
- `cargo fmt --check`
- `cargo test`
- `git diff --check`

## Product Rationale / 产品理由

中文：P0/P1 已解决诊断和配置入口。`aicmd update` 解决第三个常见断点：用户不知道如何更新，或需要复制一条很长的安装命令。

English: P0/P1 solved diagnostics and config entry points. `aicmd update` addresses the next common breakpoint: users do not know how to update or must copy a long installer command.
