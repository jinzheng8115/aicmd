# AICmd: Natural Language Terminal Commands

AICmd turns natural language into terminal commands. It is derived from [sigoden/aichat](https://github.com/sigoden/aichat), but this project focuses on one workflow: describe what you want in plain language, review the generated shell command, then run it.

AICmd 用自然语言运行终端命令。它基于 [sigoden/aichat](https://github.com/sigoden/aichat) 改造，但本项目聚焦一个工作流：用自然语言描述你要做的事，检查生成的 shell 命令，然后执行。

Original upstream / 原始上游：sigoden/aichat
License / 许可证：MIT OR Apache-2.0, following upstream licensing.

## Focus / 项目聚焦

English:
- The Rust binary is now `aicmd`.
- Default workflow: `aicmd 列出当前目录最大的 10 个文件` generates a shell command and asks before running it.
- Daily command session: plain `aicmd` uses `cmd-YYYYMMDD` by default.
- Script workflow: `aicmd-do` asks AICmd to generate commands that create and run a task script through the normal confirmation flow.
- Error workflow: `aicmd-err -- <command>` captures command output and asks AICmd to analyze it.
- Broad upstream features such as REPL, RAG, agents, macros, custom roles, and server mode are not part of the public AICmd CLI surface.

中文：
- Rust 二进制现在叫 `aicmd`。
- 默认工作流：`aicmd 列出当前目录最大的 10 个文件` 会生成 shell 命令，并在执行前让你确认。
- 每日命令会话：普通 `aicmd` 默认使用 `cmd-YYYYMMDD`。
- 脚本工作流：`aicmd-do` 会让 AICmd 生成“创建并运行任务脚本”的命令，并走正常确认流程。
- 报错工作流：`aicmd-err -- <command>` 捕获命令输出，并让 AICmd 分析。
- REPL、RAG、agents、macros、自定义 roles、server mode 等上游宽功能不属于 AICmd 的公开 CLI 使用面。

## Install / 安装

```bash
contrib/aicmd/install.sh
```

The installer builds the Rust binary and copies `aicmd`, `aicmd-do`, and `aicmd-err` to `~/.local/bin` by default.

安装脚本会构建 Rust 二进制，并默认把 `aicmd`、`aicmd-do`、`aicmd-err` 复制到 `~/.local/bin`。

## Config / 配置

English: Use `model-config.example.yaml` as the single customer-facing template for model setup. Runtime configuration is stored in `config.yaml` under the AICmd config directory. `models.yaml` is only a small internal fallback registry for maintainers.

中文：统一使用 `model-config.example.yaml` 作为唯一面向客户的模型配置模板。运行时配置保存在 AICmd 配置目录下的 `config.yaml`。`models.yaml` 只是给维护者使用的小型内部兜底模型目录。

## Detailed usage / 详细使用文档

English: See `docs/aicmd-usage.md` for the full current usage guide.

中文：完整的当前使用文档见 `docs/aicmd-usage.md`。

## Usage / 使用

```bash
# Generate and confirm a shell command / 生成并确认 shell 命令
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题

# Generate a script, review it, then run after confirmation / 生成脚本、检查、确认后执行
aicmd-do "处理 input.csv，输出 cleaned.csv"
aicmd-do --dry-run "统计 logs/*.log 里的 ERROR 数量"

# Debug a failing command / 分析报错命令
aicmd-err -- pnpm test
```

## Compatibility / 兼容性

AICmd still reuses upstream AIChat internals for model providers, config loading, sessions, roles, and shell execution. By default it uses its own `aicmd` config directory. On first startup, if no AICmd config exists but an old AIChat config exists, AICmd copies only `config.yaml` and `.env` into the new directory. Set `AICMD_CONFIG_DIR` when you want to point AICmd at another config directory explicitly.

AICmd 仍复用上游 AIChat 的 provider、配置加载、session、role 和 shell 执行内部能力。默认情况下，它使用自己的 `aicmd` 配置目录。首次启动时，如果 AICmd 配置不存在但旧 AIChat 配置存在，AICmd 只会把 `config.yaml` 和 `.env` 复制到新目录。如果你想让 AICmd 显式使用其他配置目录，请设置 `AICMD_CONFIG_DIR`。

## Upstream reference / 上游参考

See `docs/upstream-aichat.md` for upstream attribution and fork scope.

上游归属与 fork 范围说明见 `docs/upstream-aichat.md`。
