# AICmd Command Workflow / AICmd 命令工作流

English: AICmd is a command-first terminal workflow. The `aicmd` binary turns natural language into shell commands and asks before running them.

中文：AICmd 是命令优先的终端工作流。`aicmd` 二进制会把自然语言转成 shell 命令，并在执行前让你确认。

## Commands / 命令

English:
- `aicmd ...`: generate and confirm a shell command from natural language.
- `aicmd do ...`: generate commands that create and run a task script through the normal confirmation flow. `aicmd-do` remains as a compatibility wrapper.
- `aicmd err -- <command>`: capture stdout/stderr/exit code and ask AICmd to generate diagnostic/fix commands. `aicmd-err` remains as a compatibility wrapper.
- `aicmd model`: locate, show, or edit the runtime model config. `aicmd-model` remains as a compatibility wrapper.

中文：
- `aicmd ...`：从自然语言生成 shell 命令并确认执行。
- `aicmd do ...`：生成创建并运行任务脚本的命令，并走正常确认流程；`aicmd-do` 仅作为兼容 wrapper。
- `aicmd err -- <command>`：捕获 stdout/stderr/exit code，并让 AICmd 生成诊断/修复命令；`aicmd-err` 仅作为兼容 wrapper。
- `aicmd model`：定位、查看或编辑运行时模型配置；`aicmd-model` 仅作为兼容 wrapper。

## Install / 安装

Release binary install, no Rust required:

Release 二进制安装，不需要 Rust：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

Native Windows PowerShell/cmd is not supported. Windows users should install and run AICmd inside WSL with the Linux command above.

Windows 原生 PowerShell/cmd 不支持。Windows 用户请在 WSL 中使用上面的 Linux 命令安装和运行 AICmd。

Source/developer install, Rust required:

源码/开发安装，需要 Rust：

```bash
contrib/aicmd/install.sh --from-source
```

## Examples / 示例

```bash
aicmd 列出当前目录最大的 10 个文件
aicmd 把当前目录下的 png 图片压缩到 dist/images
aicmd -s dev 运行测试并修复明显问题
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd err -- pnpm test
aicmd model init
aicmd model show
```

## Design boundary / 设计边界

English: The CLI surface is focused on natural-language terminal commands. Upstream AIChat code still exists internally where needed for LLM providers, sessions, config, and shell execution, but broad workflows such as REPL, RAG, agents, macros, custom roles, and server mode are not exposed as AICmd product workflows.

中文：CLI 使用面聚焦自然语言终端命令。内部仍保留必要的上游 AIChat 代码，用于 LLM providers、session、配置和 shell 执行；但 REPL、RAG、agents、macros、自定义 roles、server mode 等宽功能不会作为 AICmd 产品工作流暴露。
