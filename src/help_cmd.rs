use anyhow::Result;

pub fn run_help_command(args: &[String]) -> Result<i32> {
    let topic = args
        .iter()
        .map(String::as_str)
        .find(|arg| !matches!(*arg, "me" | "help" | "-h" | "--help"))
        .unwrap_or("overview");
    println!("{}", help_text(topic, crate::utils::is_chinese()));
    Ok(0)
}

fn normalize_topic(topic: &str) -> String {
    let lower = topic.trim().to_lowercase();
    match lower.as_str() {
        "配置" | "模型" | "初始化" => "config".to_string(),
        "搜索" | "联网" => "search".to_string(),
        "脚本" | "安装" | "执行" => "do".to_string(),
        "会话" | "历史" => "session".to_string(),
        "总结" | "缓存" | "修复" => "fix".to_string(),
        "诊断" | "排障" => "doctor".to_string(),
        _ => lower,
    }
}

fn help_text(topic: &str, chinese: bool) -> &'static str {
    let (zh, en) = match normalize_topic(topic).as_str() {
        "setup" | "init" | "config" | "model" => (SETUP_HELP_ZH, SETUP_HELP_EN),
        "search" | "mcp" => (SEARCH_HELP_ZH, SEARCH_HELP_EN),
        "do" | "script" | "install" => (DO_HELP_ZH, DO_HELP_EN),
        "session" | "history" | "last" => (SESSION_HELP_ZH, SESSION_HELP_EN),
        "summary" | "cache" | "repair" | "fix" => (UX_HELP_ZH, UX_HELP_EN),
        "doctor" | "debug" | "troubleshoot" => (DOCTOR_HELP_ZH, DOCTOR_HELP_EN),
        _ => (OVERVIEW_HELP_ZH, OVERVIEW_HELP_EN),
    };
    if chinese {
        zh
    } else {
        en
    }
}

const OVERVIEW_HELP_ZH: &str = r#"AICmd 帮助

优先记住这些命令：
  aicmd <任务>                自动规划、确认并执行
  aicmd setup                 首次配置或重新配置
  aicmd doctor                诊断安装、配置、MCP、缓存

普通任务会自动判断为命令、脚本、搜索、错误诊断或 workflow；workflow 不是新命令。
需要环境检查、修改和最终验证时，workflow 会自动启用。只读检查会自动运行，再一次展示完整修改计划。
文件和系统修改须确认后才会执行；破坏性步骤还会二次确认。修订计划也必须重新确认。
修改步骤绝不自动重试；只有只读验证成功后，workflow 才算完成。最多生成两份修订计划。
Ctrl-C 保留已输出和聚合 session 记录，并以 130 退出；AI summary 可选，不决定 workflow 状态。

可用帮助主题：
  aicmd help setup            模型和配置
  aicmd help search           MCP 搜索
  aicmd help do               脚本任务
  aicmd help session          会话和历史
  aicmd help fix              缓存、总结、修复
  aicmd help doctor           排障

示例：
  aicmd 当前目录有多少文件
  aicmd "安装 jq，并验证安装结果"
  aicmd "读取 data/orders.csv，按用户统计订单金额，输出到 output/user_totals.csv"
  aicmd "查一下 Docker 最新安装方式"
  aicmd "分析这个报错：permission denied"
  aicmd --no-cache 当前目录有多少文件

高级显式入口：
  aicmd do "读取 data.csv，按用户统计金额"
  aicmd search "copilot-cli 如何安装"
  aicmd err -- pnpm test
"#;

const OVERVIEW_HELP_EN: &str = r#"AICmd help

Start with these commands:
  aicmd <task>                Auto-plan, confirm, and run
  aicmd setup                 First-time setup or reconfigure
  aicmd doctor                Diagnose install, config, MCP, and cache

Plain tasks are automatically classified as command, script, search, diagnosis, or workflow; workflow is not a new command.
When environment checks, changes, and final verification are needed, workflow starts automatically. Read-only checks run automatically, then AICmd shows the complete change plan once.
File and system changes run only after confirmation; destructive steps need a second confirmation. Repair plans also require renewed confirmation.
Modification steps are never retried automatically. A workflow is complete only after read-only verification succeeds. At most two repair plans are generated.
Ctrl-C preserves produced output and the aggregate session record, then exits 130. AI summary is optional and does not decide workflow status.

Useful help topics:
  aicmd help setup            Model/config setup
  aicmd help search           MCP search
  aicmd help do               Script workflow
  aicmd help session          Sessions/history
  aicmd help fix              Cache, summary, repair
  aicmd help doctor           Troubleshooting

Examples:
  aicmd how many files are in this directory
  aicmd "安装 jq，并验证安装结果"
  aicmd "read data/orders.csv, aggregate order amount by user, write output/user_totals.csv"
  aicmd "find the latest Docker installation instructions"
  aicmd "analyze this error: permission denied"
  aicmd --no-cache how many files are in this directory

Advanced explicit modes:
  aicmd do "read data.csv and aggregate amount by user"
  aicmd search "how to install copilot-cli"
  aicmd err -- pnpm test
"#;

const SETUP_HELP_ZH: &str = r#"配置帮助

推荐流程：
  1. 准备包含模型配置的 .env
  2. 运行：aicmd setup
  3. 检查：aicmd config status
  4. 诊断：aicmd doctor

常用命令：
  aicmd init --from-env          从 .env 生成 config.yaml
  aicmd init --from-env --force  二次确认后重新生成
  aicmd config status            安全查看状态，不显示密钥
  aicmd config edit              编辑运行配置
"#;

const SETUP_HELP_EN: &str = r#"Setup help

Recommended flow:
  1. Prepare .env with model settings
  2. Run: aicmd setup
  3. Check: aicmd config status
  4. Diagnose: aicmd doctor

Common commands:
  aicmd init --from-env          Generate config.yaml from .env
  aicmd init --from-env --force  Regenerate with confirmation
  aicmd config status            Safe status, no API keys
  aicmd config edit              Edit runtime config
"#;

const SEARCH_HELP_ZH: &str = r#"搜索帮助

AICmd 搜索读取 ~/.aicmd/mcp.json：先调用 MCP，再让 LLM 整理结果。

命令：
  aicmd search "查询"                  搜索并整理
  aicmd search "查询" --save 名称       保存结果
  aicmd search list                     列出搜索记录
  aicmd search show 名称                查看搜索记录
  aicmd search open 名称                打开搜索记录
  aicmd do --from-search 名称 "任务"    基于搜索结果执行
"#;

const SEARCH_HELP_EN: &str = r#"Search help

AICmd search uses ~/.aicmd/mcp.json. It calls MCP first, then asks the LLM to summarize.

Commands:
  aicmd search "query"                 Search and summarize
  aicmd search "query" --save name     Save result
  aicmd search list                     List saved searches
  aicmd search show name                Show saved result
  aicmd search open name                Open saved result
  aicmd do --from-search name "task"   Use search result for execution
"#;

const DO_HELP_ZH: &str = r#"脚本任务帮助

多步骤任务、安装流程、文件/数据处理建议使用 do。

示例：
  aicmd do "处理 input.csv，输出 cleaned.csv"
  aicmd do --plan "安装 Docker"
  aicmd do --dry-run "统计 logs/*.log 的 ERROR"
  aicmd do --from-search last "安装 copilot-cli"

AICmd 仍会在执行前询问确认。
"#;

const DO_HELP_EN: &str = r#"Do help

Use do for multi-step tasks, installs, and file/data processing.

Examples:
  aicmd do "process input.csv and write cleaned.csv"
  aicmd do --plan "install Docker"
  aicmd do --dry-run "count ERROR in logs/*.log"
  aicmd do --from-search last "install copilot-cli"

AICmd still asks before execution.
"#;

const SESSION_HELP_ZH: &str = r#"会话帮助

默认会话按日期生成，例如 cmd-YYYYMMDD。

普通 aicmd <任务> 会写入当天历史，但不会把之前内容发送给模型；需要连续上下文时使用 -s <名称>。

命令：
  aicmd -s                         显示当前默认会话
  aicmd -s dev                     进入或创建 dev 会话
  aicmd -s dev "任务"              在 dev 会话发送任务
  aicmd --list-sessions            列出会话
  aicmd -s dev --empty-session     二次确认后清空会话
  aicmd last                       查看最近历史
"#;

const SESSION_HELP_EN: &str = r#"Session help

Default session is daily, like cmd-YYYYMMDD.

Plain aicmd <task> saves history in the daily session, but does not send prior history to the model. Use -s <name> when you want a continuous conversation.

Commands:
  aicmd -s                         Show current default session
  aicmd -s dev                     Start or join dev session
  aicmd -s dev "task"              Send task in dev session
  aicmd --list-sessions            List sessions
  aicmd -s dev --empty-session     Clear selected session, with confirmation
  aicmd last                       Show recent history
"#;

const UX_HELP_ZH: &str = r#"缓存、总结、修复帮助

常用参数：
  aicmd --summary <任务>       本次强制 AI summary
  aicmd --no-summary <任务>    本次跳过 AI summary
  aicmd --no-cache <任务>      不复用成功命令缓存

AI summary 默认不自动执行。命令完成后，选择是否生成。
使用 --no-summary 跳过选择，或使用 aicmd config summary on 改为自动生成。

命令确认：
  执行？[Y/n/?]                 回车/Y 执行；N 跳过；? 显示修改、解释、复制、退出

成功命令会自动复用；按 ? 再按 g 可重新生成命令。

持久化 summary 设置：
  aicmd config summary status
  aicmd config summary off
  aicmd config summary on

如果命令失败，AICmd 会显示：
  fix | explain | copy | quit
"#;

const UX_HELP_EN: &str = r#"Cache, summary, and repair help

Useful flags:
  aicmd --summary <task>       Force AI summary once
  aicmd --no-summary <task>    Skip AI summary once
  aicmd --no-cache <task>      Do not reuse successful command cache

AI summary is not automatic by default. After execution, choose whether to generate it.
Use --no-summary to skip that choice, or aicmd config summary on to enable it automatically.

Command confirmation:
  Run? [Y/n/?]                 Enter/Y runs; N skips; ? shows revise, explain, copy, quit

Successful commands are reused automatically. Press ? then g to generate a new command.

Persistent summary setting:
  aicmd config summary status
  aicmd config summary off
  aicmd config summary on

If a command fails, AICmd shows:
  fix | explain | copy | quit
"#;

const DOCTOR_HELP_ZH: &str = r#"排障帮助

运行：
  aicmd doctor
  aicmd config status

doctor 会检查：
  binary、version、config、model、temperature、AI summary、
  MCP/search、command cache、searches dir、PATH、shell integration

如果输出看不懂，可以复制完整 doctor 输出再询问 AICmd 或开发者。
"#;

const DOCTOR_HELP_EN: &str = r#"Doctor help

Run:
  aicmd doctor
  aicmd config status

Doctor checks:
  binary, version, config, model, temperature, AI summary,
  MCP/search, command cache, searches dir, PATH, shell integration

If output is confusing, copy the full doctor output and ask AICmd or your developer.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_topic_supports_chinese_aliases() {
        assert_eq!(normalize_topic("配置"), "config");
        assert_eq!(normalize_topic("搜索"), "search");
        assert_eq!(normalize_topic("修复"), "fix");
    }

    #[test]
    fn overview_help_explains_workflow_in_the_selected_language() {
        let chinese = help_text("overview", true);
        assert!(chinese.contains("aicmd \"安装 jq，并验证安装结果\""));
        assert!(chinese.contains("只读检查会自动运行"));
        assert!(!chinese.contains("Read-only checks run automatically"));

        let english = help_text("overview", false);
        assert!(english.contains("aicmd \"安装 jq，并验证安装结果\""));
        assert!(english.contains("Read-only checks run automatically"));
        assert!(!english.contains("只读检查会自动运行"));
    }

    #[test]
    fn generated_help_has_no_markdown_backticks() {
        for topic in [
            "overview", "setup", "search", "do", "session", "fix", "doctor",
        ] {
            for chinese in [true, false] {
                let help = help_text(topic, chinese);
                assert!(
                    !help.contains('`'),
                    "{topic} help must not contain Markdown backticks"
                );
            }
        }
    }
}
