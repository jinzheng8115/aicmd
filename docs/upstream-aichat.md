# Upstream Attribution / 上游归属说明

AICmd is derived from [sigoden/aichat](https://github.com/sigoden/aichat).

AICmd 基于 [sigoden/aichat](https://github.com/sigoden/aichat) 改造。

## What this fork keeps / 本 fork 保留什么

English:
- The Rust LLM provider foundation from upstream AIChat.
- Configuration loading compatible with existing AIChat-style setups where practical.
- Session storage for command history continuity.
- Shell command generation and confirmation flow.
- Upstream license terms: MIT OR Apache-2.0.

中文：
- 上游 AIChat 的 Rust LLM provider 基础能力。
- 尽量兼容既有 AIChat 风格配置的加载方式。
- 用于命令历史连续性的 session 存储。
- shell 命令生成与确认执行流程。
- 上游许可证条款：MIT OR Apache-2.0。

## What this fork removes from the product surface / 本 fork 从产品使用面移除什么

English:
- Chat REPL mode.
- RAG workflows.
- Agents and custom tool/function workflows.
- Macros.
- Public role switching and custom role management.
- Built-in HTTP server mode.
- General-purpose model/info listing commands.

中文：
- Chat REPL 模式。
- RAG 工作流。
- Agents 与自定义 tool/function 工作流。
- Macros。
- 公开 role 切换与自定义 role 管理。
- 内置 HTTP server 模式。
- 通用 model/info 列表命令。

## Current AICmd scope / 当前 AICmd 范围

English: AICmd focuses on one workflow: describe a terminal task in natural language, review the generated shell command, then run it after confirmation. See `README.md` and `contrib/aicmd/README.md` for current usage.

中文：AICmd 聚焦一个工作流：用自然语言描述终端任务，检查生成的 shell 命令，然后确认执行。当前用法见 `README.md` 与 `contrib/aicmd/README.md`。
