pub struct RepairContext<'a> {
    pub user_task: &'a str,
    pub shell: &'a str,
    pub os: &'a str,
    pub cwd: &'a str,
    pub command: &'a str,
    pub exit_code: i32,
    pub stdout: &'a str,
    pub stderr: &'a str,
}

pub fn build_repair_prompt(context: &RepairContext<'_>) -> String {
    const OUTPUT_LIMIT: usize = 4_000;
    format!(
        "You are repairing a failed terminal command for AICmd.\n\
你正在为 AICmd 修复一条执行失败的终端命令。\n\n\
Rules / 规则:\n\
- Return one corrected shell command or script wrapper in the required JSON `command` field, with required read-only checks in `preflight`.\n\
- Do not use markdown fences.\n\
- Do not explain outside shell comments or echo/printf.\n\
- Prefer minimal changes from the failed command.\n\
- If the task is impossible or unsafe, output a safe echo command explaining why.\n\n\
Context / 上下文:\n\
User task / 用户任务: {}\n\
Shell: {}\n\
OS: {}\n\
CWD: {}\n\n\
Failed command / 失败命令:\n{}\n\n\
Exit code / 退出码: {}\n\n\
STDOUT:\n{}\n\n\
STDERR:\n{}",
        context.user_task,
        context.shell,
        context.os,
        context.cwd,
        context.command,
        context.exit_code,
        truncate_tail(context.stdout.trim(), OUTPUT_LIMIT),
        truncate_tail(context.stderr.trim(), OUTPUT_LIMIT)
    )
}

fn truncate_tail(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count == 0 {
        return "(empty)".to_string();
    }
    if count <= max_chars {
        return value.to_string();
    }
    let tail = value
        .chars()
        .skip(count.saturating_sub(max_chars))
        .collect::<String>();
    format!("[truncated / 已截断]\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_prompt_contains_bilingual_rules_and_context() {
        let prompt = build_repair_prompt(&RepairContext {
            user_task: "当前目录有多少文件",
            shell: "zsh",
            os: "macos",
            cwd: "/tmp",
            command: "bad-command",
            exit_code: 127,
            stdout: "",
            stderr: "command not found",
        });
        assert!(prompt.contains("You are repairing a failed terminal command"));
        assert!(prompt.contains("你正在为 AICmd 修复"));
        assert!(prompt.contains("User task / 用户任务: 当前目录有多少文件"));
        assert!(prompt.contains("Failed command / 失败命令:\nbad-command"));
        assert!(prompt.contains("STDERR:\ncommand not found"));
    }

    #[test]
    fn truncate_tail_keeps_tail() {
        assert_eq!(truncate_tail("abcdef", 3), "[truncated / 已截断]\ndef");
        assert_eq!(truncate_tail("", 3), "(empty)");
    }
}
