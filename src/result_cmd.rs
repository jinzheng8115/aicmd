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
        "Command execution result:\nCommand:\n{command}\n\nExit code: {code}\n\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}\n\nAI summary:\n{summary}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_execution_note() {
        let note = build_execution_session_note("printf hello", 0, "hello", "", Some("ok"));
        assert!(note.contains("Exit code: 0"));
        assert!(note.contains("STDOUT:\nhello"));
    }
}
