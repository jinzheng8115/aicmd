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

pub struct WorkflowRepairContext<'a> {
    pub user_task: &'a str,
    pub shell: &'a str,
    pub os: &'a str,
    pub cwd: &'a str,
    pub previous_plan_json: &'a str,
    pub completed_results: &'a str,
    pub failed_step: &'a str,
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

pub fn build_workflow_repair_prompt(context: &WorkflowRepairContext<'_>) -> String {
    const OUTPUT_LIMIT: usize = 4_000;
    format!(
        "Repair this failed AICmd workflow. Return a complete revised workflow plan as one strict JSON object.\n\
修复这个失败的 AICmd workflow。返回完整的修订 workflow 计划，并且只返回一个严格 JSON 对象。\n\n\
Rules / 规则:\n\
- The JSON must use mode `workflow` and satisfy the complete execution-plan schema.\n\
- Preserve already achieved outcomes where safe; do not repeat modifications merely because they succeeded before.\n\
- Include a read-only verification step for the final outcome.\n\
- Do not return Markdown fences, prose, or a single corrected command.\n\
- No command is approved until the complete revised plan is shown and confirmed again.\n\
- 在安全的前提下保留已经完成的结果，不要仅因修改步骤曾经成功就重复执行。\n\
- 必须包含最终结果的只读验证步骤。\n\
- 不要返回 Markdown、说明文字或单条修复命令。\n\
- 在完整修订计划重新展示并确认前，任何命令都未获批准。\n\n\
Original request / 原始请求: {}\n\
Shell: {}\n\
OS: {}\n\
CWD: {}\n\n\
Previous plan / 旧计划:\n{}\n\n\
Previous results / 已有结果:\n{}\n\n\
Failed step / 失败步骤: {}\n\
Exit code / 退出码: {}\n\n\
STDOUT:\n{}\n\n\
STDERR:\n{}",
        context.user_task,
        context.shell,
        context.os,
        context.cwd,
        context.previous_plan_json,
        context.completed_results,
        context.failed_step,
        context.exit_code,
        truncate_tail(context.stdout.trim(), OUTPUT_LIMIT),
        truncate_tail(context.stderr.trim(), OUTPUT_LIMIT),
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
    fn workflow_repair_prompt_contains_old_plan_and_failed_step() {
        let prompt = build_workflow_repair_prompt(&WorkflowRepairContext {
            user_task: "install tool",
            shell: "zsh",
            os: "macos",
            cwd: "/tmp",
            previous_plan_json: r#"{"mode":"workflow"}"#,
            completed_results: "check: passed",
            failed_step: "verify",
            exit_code: 1,
            stdout: "",
            stderr: "not found",
        });
        assert!(prompt.contains("Return a complete revised workflow plan"));
        assert!(prompt.contains("返回完整的修订 workflow 计划"));
        assert!(prompt.contains("Failed step / 失败步骤: verify"));
        assert!(prompt.contains("Previous plan / 旧计划"));
    }

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
