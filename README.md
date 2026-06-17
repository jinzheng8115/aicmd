# AICmd: Natural Language Terminal Commands

AICmd turns natural language into terminal commands. It is derived from [sigoden/aichat](https://github.com/sigoden/aichat), but this project focuses on one workflow: describe what you want in plain language, review the generated shell command, run it, then get an LLM summary of the command output.

AICmd 用自然语言运行终端命令。它基于 [sigoden/aichat](https://github.com/sigoden/aichat) 改造，但本项目聚焦一个工作流：用自然语言描述你要做的事，检查生成的 shell 命令，执行命令，然后让 LLM 总结命令输出。

Original upstream / 原始上游：sigoden/aichat
License / 许可证：MIT OR Apache-2.0, following upstream licensing.

## Focus / 项目聚焦

English:
- The Rust binary is now `aicmd`.
- Default workflow: `aicmd 列出当前目录最大的 10 个文件` generates a shell command and asks before running it.
- Daily command session: plain `aicmd` uses `cmd-YYYYMMDD` by default.
- Script workflow: `aicmd do` asks AICmd to generate commands that create and run a task script through the normal confirmation flow. `aicmd-do` remains as a compatibility wrapper.
- Error workflow: `aicmd err -- <command>` captures command output and asks AICmd to generate diagnostic/fix commands. `aicmd-err` remains as a compatibility wrapper.
- Broad upstream features such as REPL, RAG, agents, macros, custom roles, and server mode are not part of the public AICmd CLI surface.

中文：
- Rust 二进制现在叫 `aicmd`。
- 默认工作流：`aicmd 列出当前目录最大的 10 个文件` 会生成 shell 命令，并在执行前让你确认。
- 每日命令会话：普通 `aicmd` 默认使用 `cmd-YYYYMMDD`。
- 脚本工作流：`aicmd do` 会让 AICmd 生成“创建并运行任务脚本”的命令，并走正常确认流程；`aicmd-do` 仅作为兼容 wrapper。
- 报错工作流：`aicmd err -- <command>` 捕获命令输出，并让 AICmd 生成诊断/修复命令；`aicmd-err` 仅作为兼容 wrapper。
- 模型配置辅助命令：`aicmd model` 用于定位、查看或编辑运行时模型配置；`aicmd-model` 仅作为兼容 wrapper。
- REPL、RAG、agents、macros、自定义 roles、server mode 等上游宽功能不属于 AICmd 的公开 CLI 使用面。

## Install / 安装

Recommended first-time setup:

推荐首次安装流程：

```bash
cp .env.example .env
$EDITOR .env
contrib/aicmd/install.sh
aicmd init --from-env
```

The installer builds the Rust binary and copies `aicmd`, `aicmd-do`, `aicmd-err`, `aicmd-model`, and `aicmd-mcp` to `~/.local/bin`. After installation, run `aicmd init --from-env` to generate `~/.aicmd/config.yaml` from `.env`.

安装脚本会构建 Rust 二进制，并把 `aicmd`、`aicmd-do`、`aicmd-err`、`aicmd-model`、`aicmd-mcp` 复制到 `~/.local/bin`。安装后运行 `aicmd init --from-env`，根据 `.env` 生成 `~/.aicmd/config.yaml`。

## Config / 配置

English: Users fill `.env` before installation, then `aicmd init --from-env` generates the LLM runtime config at `~/.aicmd/config.yaml`. MCP is configured separately in `mcp.json`, which the installer copies to `~/.aicmd/mcp.json` if it does not already exist. `.env` supports exactly one model provider configuration at a time; choose `openai`, `anthropic`, or `google`. To switch provider later, edit the same `.env` file and run `aicmd init --from-env --force`. OpenAI also supports `AICMD_OPENAI_API_STYLE=chat|responses`. Temperature, max tokens, and thinking mode are not required in `.env`; defaults are used.

中文：用户在安装前填写 `.env`，然后用 `aicmd init --from-env` 生成 LLM 运行时配置 `~/.aicmd/config.yaml`。MCP 单独在 `mcp.json` 中配置；安装脚本会在 `~/.aicmd/mcp.json` 不存在时复制过去。`.env` 同一时间只支持一组模型服务配置；从 `openai`、`anthropic`、`google` 中选择一种。后续如需切换服务商，修改同一个 `.env` 文件后运行 `aicmd init --from-env --force`。OpenAI 额外支持 `AICMD_OPENAI_API_STYLE=chat|responses`。temperature、max token、thinking 模式不需要在 `.env` 中配置，默认使用关闭或默认值。

## Detailed usage / 详细使用文档

English: See `docs/aicmd-usage.md` for the full current usage guide.

中文：完整的当前使用文档见 `docs/aicmd-usage.md`。

## MCP tools / MCP 工具

AICmd keeps MCP calls separate from the main terminal-command workflow. Configure MCP servers and command mappings in `mcp.json`; the installer copies it to `~/.aicmd/mcp.json`. Command mappings only need a `server` by default. AICmd discovers MCP tools automatically; `tool` is only an advanced optional override. User-facing commands such as `aicmd search <query>` and `aicmd mcp <command> ...` call MCP first, then send the MCP result to the configured LLM for terminal-friendly summarization. The lower-level `aicmd mcp-raw <command> ...` command prints raw MCP output for debugging. `aicmd-mcp` remains as a compatibility wrapper.

AICmd 将 MCP 调用和主终端命令流程分开。MCP server 和命令映射配置在 `mcp.json`；安装脚本会复制到 `~/.aicmd/mcp.json`。命令映射默认只需要写 `server`，AICmd 会自动发现 MCP tool；`tool` 只是高级可选覆盖项。面向用户的 `aicmd search <query>` 和 `aicmd mcp <command> ...` 会先调用 MCP，再把 MCP 结果交给当前配置的 LLM 整理成适合终端阅读的输出。底层 `aicmd mcp-raw <command> ...` 会输出 MCP 原始结果，方便调试；`aicmd-mcp` 仅作为兼容 wrapper。

```bash
aicmd init --from-env

aicmd search "今天 AI 新闻"
aicmd search DeepSeek latest model

# Underlying helper / 底层辅助命令
aicmd mcp-raw search "今天 AI 新闻"
aicmd mcp-raw tavily "DeepSeek latest model"
```

## Shell integration / Shell 集成

The installer adds shell integration to your shell rc file automatically, so new terminals can update the current directory after AICmd executes commands such as `cd ..`.

安装脚本会自动把 shell 集成写入你的 shell rc 文件，因此新打开的终端在 AICmd 执行 `cd ..` 这类命令后可以同步更新当前目录。

For the current already-open terminal, run once:

对于当前已经打开的终端，需要执行一次：

```bash
source ~/.zshrc
```

Or enable it manually for the current shell:

也可以只在当前 shell 手动启用：

```bash
eval "$(aicmd-shell-init)"
```

## Usage / 使用

```bash
# Generate and confirm a shell command / 生成并确认 shell 命令
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题

# Generate a script, review it, then run after confirmation / 生成脚本、检查、确认后执行
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --dry-run "统计 logs/*.log 里的 ERROR 数量"

# Search with MCP / 使用 MCP 搜索
aicmd search "今天 AI 新闻"

# Debug a failing command / 分析报错命令
aicmd err -- pnpm test
```

## Compatibility / 兼容性

AICmd still reuses upstream AIChat internals for model providers, config loading, sessions, roles, and shell execution. By default it uses the visible `~/.aicmd` config directory. On first startup, if no AICmd config exists but an older AICmd or AIChat config exists, AICmd copies only `config.yaml` and `.env` into `~/.aicmd`. Set `AICMD_CONFIG_DIR` when you want to point AICmd at another config directory explicitly.

AICmd 仍复用上游 AIChat 的 provider、配置加载、session、role 和 shell 执行内部能力。默认情况下，它使用用户可见的 `~/.aicmd` 配置目录。首次启动时，如果 AICmd 配置不存在但旧版 AICmd 或 AIChat 配置存在，AICmd 只会把 `config.yaml` 和 `.env` 复制到 `~/.aicmd`。如果你想让 AICmd 显式使用其他配置目录，请设置 `AICMD_CONFIG_DIR`。

## Upstream reference / 上游参考

See `docs/upstream-aichat.md` for upstream attribution and fork scope.

上游归属与 fork 范围说明见 `docs/upstream-aichat.md`。
