# AICmd Command Workflow / AICmd 命令工作流

English: AICmd is a command-first terminal workflow. The `aicmd` binary turns natural language into shell commands and asks before running them.

中文：AICmd 是命令优先的终端工作流。`aicmd` 二进制会把自然语言转成 shell 命令，并在执行前让你确认。

## Commands / 命令

English:
- `aicmd ...`: generate and confirm a shell command from natural language.
- `aicmd-do ...`: generate commands that create and run a task script through the normal confirmation flow.
- `aicmd-err -- <command>`: capture stdout/stderr/exit code and ask AICmd to analyze the failure.

中文：
- `aicmd ...`：从自然语言生成 shell 命令并确认执行。
- `aicmd-do ...`：生成创建并运行任务脚本的命令，并走正常确认流程。
- `aicmd-err -- <command>`：捕获 stdout/stderr/exit code，并让 AICmd 分析失败原因。

## Install / 安装

```bash
contrib/aicmd/install.sh
```

The installer builds the Rust binary and copies `aicmd`, `aicmd-do`, and `aicmd-err` to `~/.local/bin` by default.

安装脚本会构建 Rust 二进制，并默认把 `aicmd`、`aicmd-do`、`aicmd-err` 复制到 `~/.local/bin`。

## Examples / 示例

```bash
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题
aicmd-do "处理 input.csv，输出 cleaned.csv"
aicmd-err -- pnpm test
```

## Design boundary / 设计边界

English: The CLI surface is now focused on natural-language terminal commands. Upstream AIChat code still exists internally where needed for LLM providers, sessions, config, and shell execution, but broad workflows such as REPL, RAG, agents, macros, and server mode are no longer product goals.

中文：CLI 使用面现在聚焦自然语言终端命令。内部仍保留必要的上游 AIChat 代码，用于 LLM providers、session、配置和 shell 执行；但 REPL、RAG、agents、macros、server mode 等宽功能不再是产品目标。
