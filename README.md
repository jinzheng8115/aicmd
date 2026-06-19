# AICmd

[English](README.en.md)

AICmd 是一个用自然语言运行终端命令的工具。你描述目标，AICmd 生成可检查的命令或脚本；你确认后执行，并可选择让 LLM 总结执行结果。

上游来源：[sigoden/aichat](https://github.com/sigoden/aichat)
许可证：MIT OR Apache-2.0

## 1. 你只需要先记住这 5 个入口

```bash
aicmd <你想做的事>           # 生成一条命令，确认后执行
aicmd do <复杂任务>          # 多步骤、脚本、文件处理任务
aicmd search <需要查询的问题> # 通过 MCP 搜索，再由 LLM 整理
aicmd setup                 # 首次配置或重新配置
aicmd doctor                # 检查安装、模型、MCP、PATH、shell 集成
```

常见例子：

```bash
aicmd 当前目录有多少文件
aicmd 列出当前目录最大的 10 个文件
aicmd do "读取 data/orders.csv，按用户统计金额，输出 output/user_totals.csv"
aicmd search "copilot-cli 如何安装"
```

## 2. 支持平台

AICmd 正式支持 macOS、Linux 和 Windows WSL。不支持 Windows 原生 PowerShell/cmd。

| 系统 | 架构 |
| --- | --- |
| macOS | Apple Silicon / Intel |
| Linux / WSL | ARM64 / x86_64 |

## 3. 安装

推荐二进制安装，不需要安装 Rust：

```bash
curl -fsSL https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh | bash
```

默认位置：

| 内容 | 路径 |
| --- | --- |
| 主程序 | `~/.local/bin/aicmd` |
| 模型配置 | `~/.aicmd/config.yaml` |
| MCP 配置 | `~/.aicmd/mcp.json` |
| 会话记录 | `~/.aicmd/sessions` |
| 搜索记录 | `~/.aicmd/searches` |
| 成功命令缓存 | `~/.aicmd/command-cache.yaml` |

如果已经把项目 clone 到本地，也可以直接运行仓库里的安装脚本。默认仍然是二进制安装，会下载 GitHub Release，不需要 Rust：

```bash
git clone https://github.com/jinzheng8115/aicmd.git
cd aicmd
contrib/aicmd/install.sh
```

## 4. 首次配置

### 4.1 准备 `.env`

AICmd 需要模型配置。最小 OpenAI 兼容示例：

```env
AICMD_MODEL_NAME=deepseek
AICMD_MODEL_PROVIDER=openai
AICMD_MODEL_API_BASE=https://api.deepseek.com/v1
AICMD_MODEL_API_KEY=sk-xxxx
AICMD_MODEL_IDS=deepseek-chat
AICMD_OPENAI_API_STYLE=chat
```

支持的 `AICMD_MODEL_PROVIDER`：

| Provider | 说明 |
| --- | --- |
| `openai` | OpenAI 或兼容接口；`AICMD_OPENAI_API_STYLE=chat` 或 `responses` |
| `anthropic` | Anthropic Claude |
| `google` | Google Gemini |

可选默认模型：

```env
AICMD_DEFAULT_MODEL=deepseek:deepseek-chat
```

如果不填，默认使用：

```text
AICMD_MODEL_NAME:AICMD_MODEL_IDS 中的第一个模型
```

### 4.2 生成配置

推荐运行：

```bash
aicmd setup
```

或者直接从 `.env` 生成：

```bash
aicmd init --from-env
```

如果配置已存在，需要覆盖：

```bash
aicmd init --from-env --force
```

生成的 `config.yaml` 默认包含：

```yaml
temperature: 0
top_p: null
stream: false
ai_summary: true
```

默认开启命令执行后的 AI summary。如果想默认关闭：

```bash
aicmd config summary off
```

临时关闭一次：

```bash
aicmd --no-summary 当前目录有多少文件
```

检查配置：

```bash
aicmd doctor
aicmd config status
aicmd config path
aicmd config show
aicmd config edit
```

## 5. MCP 和搜索

MCP 配置文件：

```text
~/.aicmd/mcp.json
```

最小示例：

```json
{
  "mcp": {
    "servers": {
      "tavily": {
        "type": "stdio",
        "command": "npx",
        "args": ["-y", "tavily-mcp"],
        "env": {
          "TAVILY_API_KEY": "tvly-xxxx"
        }
      }
    },
    "commands": {
      "search": {
        "description": "Search the web using Tavily",
        "server": "tavily"
      }
    }
  }
}
```

搜索用法：

```bash
aicmd search "今天 AI 新闻"
aicmd search "gemini-cli 官方安装方式" --save gemini-cli
aicmd search list
aicmd search show gemini-cli
aicmd search open gemini-cli
aicmd search rm gemini-cli
```

交互式终端里，搜索完成后会出现：

```text
save(保存) | do(基于结果执行) | open(打开) | quit(退出):
```

- `save`：保存搜索结果。
- `do`：基于搜索结果和当前系统环境生成执行脚本。
- `open`：打开最近搜索记录。

## 6. 常用命令

### 6.1 普通命令

```bash
aicmd 当前目录有多少文件
aicmd --print 当前目录有多少文件      # 只打印命令，不执行
aicmd --dry-run 当前目录有多少文件    # 查看将发送给模型的 prompt
aicmd --no-summary 当前目录有多少文件 # 执行后跳过 AI summary
aicmd --no-cache 当前目录有多少文件   # 不复用之前成功的命令
```

执行前会出现：

```text
execute(执行) | revise(修改) | describe(解释) | copy(复制) | quit(退出):
```

AICmd 会显示风险等级。高风险命令会要求二次确认。

如果同一个普通任务之前成功执行过，AICmd 可能会先提示复用之前的命令，减少同一句话生成不同命令的问题：

```text
Found a previously successful command / 找到一条之前成功执行过的命令:
reuse(复用) | new(重新生成) | describe(解释) | quit(退出):
```

如果命令执行失败，AICmd 会提供失败处理菜单。`fix` 会基于失败命令、exit code、stdout/stderr 和当前系统环境生成修复命令，但仍需要你确认后才会执行：

```text
fix(修复) | explain(解释) | copy(复制) | quit(退出):
```

### 6.2 脚本任务：`aicmd do`

适合多步骤任务、文件处理、安装流程：

```bash
aicmd do "处理 input.csv，输出 cleaned.csv"
aicmd do --plan "安装 Docker"                    # 只生成计划，不执行
aicmd do --dry-run "统计 logs/*.log 的 ERROR"    # 查看 do prompt
aicmd do -f notes.txt "按说明执行"
aicmd do --from-search gemini-cli "安装 gemini-cli"
aicmd do -o scripts/task.sh "清洗 CSV"
```

`--from-search` 会读取 `~/.aicmd/searches/<name>.txt`，并附带当前系统环境，例如 OS、架构、当前目录、`brew/node/npm/git/curl` 是否存在，用于生成更可靠的脚本。

### 6.3 报错诊断：`aicmd err`

```bash
aicmd err -- pnpm test
aicmd err -- python scripts/import.py data.csv
```

它会真实执行命令，捕获 stdout/stderr/exit code，然后让 LLM 生成诊断或修复命令。

### 6.4 会话

```bash
aicmd -s                    # 显示当前默认 session
aicmd -s dev                # 进入或创建 dev session
aicmd -s dev hello          # 用 dev session 发送请求
aicmd --list-sessions       # 列出 session
aicmd -s dev --empty-session # 清空 dev session，会二次确认
```

默认情况下，普通 `aicmd ...` 使用每日 session，例如 `cmd-20260619`。

查看历史：

```bash
aicmd session
aicmd session list
aicmd session show
aicmd session show dev --limit 5
aicmd last
```

### 6.5 配置命令

```bash
aicmd config init            # 从 .env 生成 config.yaml
aicmd config init --force    # 覆盖已有配置，会二次确认
aicmd config status          # 安全查看当前模型、温度、summary、MCP、session 状态
aicmd config path            # 输出 config.yaml 路径
aicmd config dir             # 输出 ~/.aicmd 目录
aicmd config show            # 输出 config.yaml，注意可能包含 API key
aicmd config edit            # 用 $EDITOR 编辑 config.yaml
aicmd config summary status  # 查看 AI summary 默认开关
aicmd config summary off     # 默认关闭 AI summary
aicmd config summary on      # 默认开启 AI summary
aicmd config mcp             # 输出 mcp.json 路径
aicmd config doctor          # 等同于 aicmd doctor
```

### 6.6 更新

```bash
aicmd update --check
aicmd update
aicmd update --version v0.4.0
aicmd update --dry-run
```

## 7. Shell 集成

Shell 集成用于让 `cd ..` 这类命令影响当前终端目录。正常安装后通常已经自动配置。特殊情况下可手动启用：

```bash
eval "$(aicmd shell-init)"
```

如果没有 shell 集成，AICmd 仍能执行命令，但 `cd` 的结果不会同步到当前终端。

## 8. 安全注意事项

- 执行前一定要检查生成的命令。
- 对 `rm`、`mv`、`chmod`、数据库迁移、云资源操作等命令保持谨慎。
- `.env`、`~/.aicmd/config.yaml`、`~/.aicmd/mcp.json` 可能包含 API key，不要提交到公开仓库。
- MCP server 会运行本地命令，例如 `npx ...`，只配置你信任的 MCP server。
- `aicmd err -- <command>` 会真实执行传入命令。

## 9. 常见问题

### `aicmd: command not found`

```bash
export PATH="$HOME/.local/bin:$PATH"
hash -r
```

然后新开终端或重新运行命令。

### 找不到配置

```bash
aicmd doctor
aicmd config path
aicmd init --from-env
```

### 修改 `.env` 后模型没有变化

重新生成配置：

```bash
aicmd init --from-env --force
```

### MCP 搜索超时

首次运行 `npx -y ...` 可能需要下载 MCP 包。可以临时调大超时：

```bash
AICMD_MCP_START_TIMEOUT_SECS=300 AICMD_MCP_CALL_TIMEOUT_SECS=600 aicmd search "今天北京天气"
```

也可以检查：

```bash
aicmd config mcp
aicmd mcp list
```

### `cd ..` 执行后目录没变

启用 shell 集成，或安装后新开终端：

```bash
eval "$(aicmd shell-init)"
```

## 10. 项目范围

AICmd 复用上游 AIChat 的模型客户端、session、role 和 shell 执行能力，但公开 CLI 聚焦自然语言终端命令工作流。

更多上游说明：`docs/upstream-aichat.md`
