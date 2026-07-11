use anyhow::Result;

pub fn run_help_command(args: &[String]) -> Result<i32> {
    let topic = args
        .iter()
        .map(String::as_str)
        .find(|arg| !matches!(*arg, "me" | "help" | "-h" | "--help"))
        .unwrap_or("overview");
    print_topic(topic);
    Ok(0)
}

fn print_topic(topic: &str) {
    match normalize_topic(topic).as_str() {
        "setup" | "init" | "config" | "model" => print_setup_help(),
        "search" | "mcp" => print_search_help(),
        "do" | "script" | "install" => print_do_help(),
        "session" | "history" | "last" => print_session_help(),
        "summary" | "cache" | "repair" | "fix" => print_ux_help(),
        "doctor" | "debug" | "troubleshoot" => print_doctor_help(),
        _ => print_overview_help(),
    }
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

fn print_overview_help() {
    println!(
        r#"AICmd help / AICmd 帮助

Start with these commands / 优先记住这些命令:
  aicmd <task>              Auto-plan, confirm, and run / 自动规划、确认并执行
  aicmd setup               First-time setup or reconfigure / 首次配置或重新配置
  aicmd doctor              Diagnose install/config/MCP/cache / 诊断安装、配置、MCP、缓存

Plain tasks are automatically classified as command, script, search, or diagnosis.
普通任务会自动判断为命令、脚本、搜索或错误诊断。

Useful help topics / 可用帮助主题:
  aicmd help setup          Model/config setup / 模型和配置
  aicmd help search         MCP search / MCP 搜索
  aicmd help do             Script workflow / 脚本任务
  aicmd help session        Sessions/history / 会话和历史
  aicmd help fix            Cache, summary, repair / 缓存、总结、修复
  aicmd help doctor         Troubleshooting / 排障

Examples / 示例:
  aicmd 当前目录有多少文件
  aicmd "读取 data/orders.csv，按用户统计订单金额，输出到 output/user_totals.csv"
  aicmd "查一下 Docker 最新安装方式"
  aicmd "分析这个报错：permission denied"
  aicmd --no-cache 当前目录有多少文件

Advanced explicit modes / 高级显式入口:
  aicmd do "读取 data.csv，按用户统计金额"
  aicmd search "copilot-cli 如何安装"
  aicmd err -- pnpm test
"#
    );
}

fn print_setup_help() {
    println!(
        r#"Setup help / 配置帮助

Recommended flow / 推荐流程:
  1. Prepare .env with model settings / 准备包含模型配置的 .env
  2. Run: aicmd setup
  3. Check: aicmd config status
  4. Diagnose: aicmd doctor

Common commands / 常用命令:
  aicmd init --from-env          Generate config.yaml from .env / 从 .env 生成配置
  aicmd init --from-env --force  Regenerate with confirmation / 二次确认后重新生成
  aicmd config status            Safe status, no API keys / 安全查看状态，不显示密钥
  aicmd config edit              Edit runtime config / 编辑运行配置
"#
    );
}

fn print_search_help() {
    println!(
        r#"Search help / 搜索帮助

AICmd search uses ~/.aicmd/mcp.json. It calls MCP first, then asks the LLM to summarize.
AICmd 搜索读取 ~/.aicmd/mcp.json：先调用 MCP，再让 LLM 整理结果。

Commands / 命令:
  aicmd search "query"                 Search and summarize / 搜索并整理
  aicmd search "query" --save name     Save result / 保存结果
  aicmd search list                     List saved searches / 列出搜索记录
  aicmd search show name                Show saved result / 查看搜索记录
  aicmd search open name                Open saved result / 打开搜索记录
  aicmd do --from-search name "task"   Use search result for execution / 基于搜索结果执行
"#
    );
}

fn print_do_help() {
    println!(
        r#"Do help / 脚本任务帮助

Use `do` for multi-step tasks, installs, and file/data processing.
多步骤任务、安装流程、文件/数据处理建议使用 `do`。

Examples / 示例:
  aicmd do "处理 input.csv，输出 cleaned.csv"
  aicmd do --plan "安装 Docker"
  aicmd do --dry-run "统计 logs/*.log 的 ERROR"
  aicmd do --from-search last "安装 copilot-cli"

AICmd still asks before execution. / AICmd 仍会在执行前询问确认。
"#
    );
}

fn print_session_help() {
    println!(
        r#"Session help / 会话帮助

Default session is daily, like cmd-YYYYMMDD.
默认会话按日期生成，例如 cmd-YYYYMMDD。

Plain `aicmd <task>` saves history in the daily session, but does not send prior
history to the model. Use `-s <name>` when you want a continuous conversation.
普通 `aicmd <任务>` 会写入当天历史，但不会把之前内容发送给模型；需要连续上下文时使用 `-s <名称>`。

Commands / 命令:
  aicmd -s                         Show current default session / 显示当前默认会话
  aicmd -s dev                     Start or join dev session / 进入或创建 dev 会话
  aicmd -s dev "task"              Send task in dev session / 在 dev 会话发送任务
  aicmd --list-sessions            List sessions / 列出会话
  aicmd -s dev --empty-session     Clear selected session, with confirmation / 二次确认后清空会话
  aicmd last                       Show recent history / 查看最近历史
"#
    );
}

fn print_ux_help() {
    println!(
        r#"Cache, summary, and repair help / 缓存、总结、修复帮助

Useful flags / 常用参数:
  aicmd --summary <task>       Force AI summary once / 本次强制 AI summary
  aicmd --no-summary <task>    Skip AI summary once / 本次跳过 AI summary
  aicmd --no-cache <task>      Do not reuse successful command cache / 不复用成功命令缓存

AI summary is not automatic by default. After execution, choose whether to generate it.
AI summary 默认不自动执行。命令完成后，选择是否生成。
Use `--no-summary` to skip that choice, or `aicmd config summary on` to enable it automatically.
使用 `--no-summary` 跳过选择，或使用 `aicmd config summary on` 改为自动生成。

Command confirmation / 命令确认:
  Run? [Y/n/?]                 Enter/Y runs; N skips; ? shows revise, explain, copy, quit
  执行？[Y/n/?]                 回车/Y 执行；N 跳过；? 显示修改、解释、复制、退出

Successful commands are reused automatically. Press ? then g to generate a new command.
成功命令会自动复用；按 ? 再按 g 可重新生成命令。

Persistent summary setting / 持久化 summary 设置:
  aicmd config summary status
  aicmd config summary off
  aicmd config summary on

If a command fails, AICmd shows / 如果命令失败，AICmd 会显示:
  fix(修复) | explain(解释) | copy(复制) | quit(退出)
"#
    );
}

fn print_doctor_help() {
    println!(
        r#"Doctor help / 诊断帮助

Run / 运行:
  aicmd doctor
  aicmd config status

Doctor checks / doctor 会检查:
  binary, version, config, model, temperature, AI summary,
  MCP/search, command cache, searches dir, PATH, shell integration

If output is confusing, copy the full doctor output and ask AICmd or your developer.
如果输出看不懂，可以复制完整 doctor 输出再询问 AICmd 或开发者。
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_topic_supports_chinese_aliases() {
        assert_eq!(normalize_topic("配置"), "config");
        assert_eq!(normalize_topic("搜索"), "search");
        assert_eq!(normalize_topic("修复"), "fix");
    }
}
