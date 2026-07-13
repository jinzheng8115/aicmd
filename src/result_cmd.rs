use crate::{
    confirm_cmd,
    plan_cmd::{WorkflowConditionResult, WorkflowRisk},
    preflight_cmd::PreflightReport,
    utils::{color_text, dimmed_text, localized},
    workflow_cmd::{StepStatus, WorkflowRecord},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureAction {
    Repair,
    Explain,
    Copy,
    Quit,
}

pub fn failure_actions(repair_attempts: u8) -> Vec<FailureAction> {
    let mut actions = vec![
        FailureAction::Explain,
        FailureAction::Copy,
        FailureAction::Quit,
    ];
    if repair_attempts < 2 {
        actions.insert(0, FailureAction::Repair);
    }
    actions
}

pub fn prompt_failure_action(repair_attempts: u8) -> anyhow::Result<FailureAction> {
    let actions = failure_actions(repair_attempts);
    if !actions.contains(&FailureAction::Repair) {
        println!(
            "{}",
            dimmed_text(localized(
                "已达到自动修复次数上限。请手动检查错误，或修改任务描述。",
                "Repair limit reached. Please inspect the error manually or revise the task.",
            ))
        );
    }
    let (keys, labels): (Vec<_>, Vec<_>) = actions
        .iter()
        .map(|action| match action {
            FailureAction::Repair => ('f', localized(" 修复", "ix")),
            FailureAction::Explain => ('e', localized(" 解释", "xplain")),
            FailureAction::Copy => ('c', localized(" 复制", "opy")),
            FailureAction::Quit => ('q', localized(" 退出", "uit")),
        })
        .unzip();
    let options = keys
        .iter()
        .zip(labels)
        .map(|(key, label)| color_text(&key.to_string(), nu_ansi_term::Color::Cyan) + label)
        .collect::<Vec<_>>();
    let prompt = format!(
        "{}。{}: ",
        localized("命令执行失败", "Command failed"),
        options.join(&dimmed_text(" | "))
    );
    let choice = confirm_cmd::read_action(&keys, 'e', &prompt)?;
    Ok(match choice {
        'f' => FailureAction::Repair,
        'e' => FailureAction::Explain,
        'c' => FailureAction::Copy,
        _ => FailureAction::Quit,
    })
}

pub fn truncate_for_session(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push_str("\n[truncated / 已截断]");
    }
    if out.trim().is_empty() {
        "(empty)".to_string()
    } else {
        out
    }
}

pub fn build_execution_session_note(
    command: &str,
    code: i32,
    termination: &str,
    stdout: &str,
    stderr: &str,
    summary: Option<&str>,
) -> String {
    const OUTPUT_LIMIT: usize = 4_000;
    const SUMMARY_LIMIT: usize = 2_000;
    let stdout = truncate_for_session(stdout.trim(), OUTPUT_LIMIT);
    let stderr = truncate_for_session(stderr.trim(), OUTPUT_LIMIT);
    let summary = summary
        .map(|value| truncate_for_session(value.trim(), SUMMARY_LIMIT))
        .unwrap_or_else(|| "(empty)".to_string());
    format!(
        "Command execution result:\nCommand:\n{command}\n\nExit code: {code}\nTermination: {termination}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}\n\nAI summary:\n{summary}"
    )
}

pub fn build_preflight_session_note(task: &str, report: &PreflightReport) -> String {
    let failures = report
        .failures
        .iter()
        .map(|failure| {
            format!(
                "- {:?}: {} | {}",
                failure.check.kind, failure.check.value, failure.check.failure_message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Execution preflight failed:\nTask:\n{task}\n\nFailures:\n{failures}")
}

pub fn build_workflow_session_note(record: &WorkflowRecord) -> String {
    const OUTPUT_LIMIT: usize = 4_000;
    let plan = serde_json::to_string_pretty(&record.plan)
        .unwrap_or_else(|error| format!("(plan serialization failed: {error})"));
    let results = record
        .results
        .iter()
        .map(|result| {
            let step = record
                .plan
                .steps
                .iter()
                .find(|step| step.id == result.step_id);
            let skip_reason = if result.status == StepStatus::Skipped {
                step
                    .and_then(|step| step.run_if.as_ref())
                    .map(|condition| {
                        let expected = match condition.result {
                            WorkflowConditionResult::Passed => "passed",
                            WorkflowConditionResult::Failed => "failed",
                        };
                        format!(
                            "\nSkip reason: condition {}={} was not met",
                            condition.step, expected
                        )
                    })
                    .unwrap_or_else(|| "\nSkip reason: not eligible".to_string())
            } else {
                String::new()
            };
            let command = step.map(|step| step.command.as_str()).unwrap_or("(unknown)");
            let risk = step
                .map(|step| {
                    match confirm_cmd::effective_workflow_risk(&step.command, step.risk) {
                    WorkflowRisk::ReadOnly => "read_only",
                    WorkflowRisk::ChangesFiles => "changes_files",
                    WorkflowRisk::ChangesSystem => "changes_system",
                    WorkflowRisk::Destructive => "destructive",
                    }
                })
                .unwrap_or("unknown");
            format!(
                "Step: {}\nCommand: {}\nRisk: {}\nStatus: {}\nExit code: {}\nTermination: {}{}\nSTDOUT:\n{}\nSTDERR:\n{}",
                result.step_id,
                command,
                risk,
                result.status.as_str(),
                result.exit_code,
                result.termination,
                skip_reason,
                truncate_for_session(result.stdout.trim(), OUTPUT_LIMIT),
                truncate_for_session(result.stderr.trim(), OUTPUT_LIMIT),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "Workflow execution result:\nRequest:\n{}\n\nConfirmed plan:\n{}\n\nOrdered results:\n{}\n\nRepair attempts: {}\nWorkflow status: {}",
        record.request,
        plan,
        results,
        record.repair_attempts,
        record.status.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan_cmd::parse_execution_plan,
        workflow_cmd::{StepResult, StepStatus, WorkflowRecord, WorkflowStatus},
    };

    const THREE_STEP_WORKFLOW_JSON: &str = r#"{
      "mode":"workflow","command":"","query":"","problem":"","preflight":[],
      "summary":"Install tool",
      "steps":[
        {"id":"check","kind":"check","command":"command -v tool","risk":"read_only","on_failure":"continue"},
        {"id":"install","kind":"action","command":"brew install tool","risk":"changes_system","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
        {"id":"verify","kind":"verify","command":"tool --version","risk":"read_only","on_failure":"repair"}
      ]
    }"#;

    fn workflow_record_fixture(status: WorkflowStatus) -> WorkflowRecord {
        let plan = parse_execution_plan(THREE_STEP_WORKFLOW_JSON)
            .unwrap()
            .workflow()
            .unwrap();
        let verify = if status == WorkflowStatus::Cancelled {
            StepResult {
                step_id: "verify".to_string(),
                status: StepStatus::Cancelled,
                exit_code: 130,
                termination: "cancelled".to_string(),
                stdout: "partial output".to_string(),
                stderr: String::new(),
            }
        } else {
            StepResult::exited("verify", 0, "tool 1.0", "")
        };
        WorkflowRecord {
            request: "Install tool".to_string(),
            plan,
            results: vec![
                StepResult::exited("check", 0, "/usr/bin/tool", ""),
                StepResult {
                    step_id: "install".to_string(),
                    status: StepStatus::Skipped,
                    exit_code: 0,
                    termination: "not_run".to_string(),
                    stdout: String::new(),
                    stderr: String::new(),
                },
                verify,
            ],
            repair_attempts: 0,
            status,
        }
    }

    #[test]
    fn repair_is_unavailable_after_two_attempts() {
        assert!(failure_actions(0).contains(&FailureAction::Repair));
        assert!(!failure_actions(2).contains(&FailureAction::Repair));
    }

    #[test]
    fn builds_execution_note() {
        let note =
            build_execution_session_note("printf hello", 0, "exited", "hello", "", Some("ok"));
        assert!(note.contains("Exit code: 0"));
        assert!(note.contains("Termination: exited"));
        assert!(note.contains("STDOUT:\nhello"));
    }

    #[test]
    fn builds_preflight_failure_note_without_environment_values() {
        use crate::preflight_cmd::{
            PreflightCheck, PreflightFailure, PreflightReport, PreflightType,
        };

        let report = PreflightReport {
            total: 1,
            failures: vec![PreflightFailure {
                check: PreflightCheck {
                    kind: PreflightType::EnvExists,
                    value: "API_TOKEN".to_string(),
                    failure_message: "缺少环境变量".to_string(),
                    suggestion: "请配置 API_TOKEN".to_string(),
                },
                detail: "not set".to_string(),
            }],
        };
        let note = build_preflight_session_note("deploy", &report);
        assert!(note.contains("API_TOKEN"));
        assert!(!note.contains("secret-value"));
    }

    #[test]
    fn workflow_note_contains_plan_steps_and_final_status() {
        let record = workflow_record_fixture(WorkflowStatus::Completed);
        let note = build_workflow_session_note(&record);
        assert!(note.contains("Workflow status: completed"));
        assert!(note.contains("Step: check"));
        assert!(note.contains("Step: verify"));
        assert!(note.contains("Exit code: 0"));
    }

    #[test]
    fn cancelled_workflow_note_keeps_partial_output() {
        let record = workflow_record_fixture(WorkflowStatus::Cancelled);
        let note = build_workflow_session_note(&record);
        assert!(note.contains("Workflow status: cancelled"));
        assert!(note.contains("partial output"));
    }
}
