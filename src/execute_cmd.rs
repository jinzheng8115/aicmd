use crate::utils::Shell;
use crate::utils::{localized, AbortSignal};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::{
    env,
    io::{self, Read, Write},
    process::{Command, ExitStatus, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[cfg(not(windows))]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn cmd_double_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[cfg(windows)]
fn powershell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn with_cwd_capture(shell: &Shell, command: &str) -> String {
    let Ok(cwd_file) = env::var("AICMD_CWD_FILE") else {
        return command.to_string();
    };
    if cwd_file.is_empty() {
        return command.to_string();
    }
    #[cfg(not(windows))]
    let _ = shell;
    #[cfg(windows)]
    {
        if matches!(shell.name.as_str(), "powershell" | "pwsh") {
            return format!(
                "& {{\r\n{command}\r\n$__aicmd_status = if ($null -ne $LASTEXITCODE) {{ $LASTEXITCODE }} else {{ 0 }}\r\n(Get-Location).Path | Set-Content -LiteralPath {} -Encoding UTF8\r\nexit $__aicmd_status\r\n}}",
                powershell_single_quote(&cwd_file)
            );
        }
        format!(
            "{command}\r\nset __aicmd_status=%ERRORLEVEL%\r\ncd > {}\r\nexit /b %__aicmd_status%",
            cmd_double_quote(&cwd_file)
        )
    }
    #[cfg(not(windows))]
    format!(
        "{{\n{command}\n}}\n__aicmd_status=$?\npwd > {}\nexit $__aicmd_status",
        shell_single_quote(&cwd_file)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub termination: CommandTermination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTermination {
    Exited,
    Cancelled,
}

impl CommandTermination {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::Cancelled => "cancelled",
        }
    }
}

pub async fn run_command_capture_controlled(
    shell: &Shell,
    command: &str,
    abort_signal: AbortSignal,
) -> Result<CommandOutput> {
    eprintln!(
        "{}",
        localized(
            "命令执行中 · Ctrl-C 取消",
            "Command running · Ctrl-C to cancel"
        )
    );
    let shell_cmd = shell.cmd.clone();
    let shell_arg = shell.arg.clone();
    let command = command.to_string();
    let worker_abort = abort_signal.clone();
    let mut worker = tokio::task::spawn_blocking(move || {
        run_command_capture_blocking(&shell_cmd, &shell_arg, &command, worker_abort)
    });
    tokio::select! {
        result = &mut worker => result.context("command worker failed")?,
        _ = tokio::signal::ctrl_c() => {
            abort_signal.set_ctrlc();
            worker.await.context("command worker failed")?
        }
    }
}

fn run_command_capture_blocking(
    shell_cmd: &str,
    shell_arg: &str,
    command: &str,
    abort_signal: AbortSignal,
) -> Result<CommandOutput> {
    let mut child = Command::new(shell_cmd)
        .args([shell_arg, command])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;
    let activity = Arc::new(Mutex::new(Instant::now()));
    let stdout_activity = activity.clone();
    let stderr_activity = activity.clone();
    let stdout_handle = thread::spawn(move || stream_and_capture(stdout, false, stdout_activity));
    let stderr_handle = thread::spawn(move || stream_and_capture(stderr, true, stderr_activity));
    let started = Instant::now();
    let mut last_heartbeat = started;
    let mut cancel_started = None;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if abort_signal.aborted() {
            let cancel_started = cancel_started.get_or_insert_with(|| {
                signal_process_tree(child.id(), "INT");
                Instant::now()
            });
            if cancel_started.elapsed() >= Duration::from_secs(2) {
                signal_process_tree(child.id(), "KILL");
                let _ = child.kill();
            }
        }
        let now = Instant::now();
        if now.duration_since(*activity.lock()) >= Duration::from_secs(5)
            && now.duration_since(last_heartbeat) >= Duration::from_secs(5)
        {
            eprintln!(
                "{}",
                if crate::utils::is_chinese() {
                    format!(
                        "命令仍在执行 · {} 秒 · Ctrl-C 取消",
                        started.elapsed().as_secs()
                    )
                } else {
                    format!(
                        "Command still running · {}s · Ctrl-C to cancel",
                        started.elapsed().as_secs()
                    )
                }
            );
            last_heartbeat = now;
        }
        thread::sleep(Duration::from_millis(100));
    };
    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;
    let cancelled = abort_signal.aborted() || status_was_cancelled(&status);
    Ok(CommandOutput {
        code: if cancelled {
            130
        } else {
            status.code().unwrap_or(1)
        },
        stdout,
        stderr,
        termination: if cancelled {
            CommandTermination::Cancelled
        } else {
            CommandTermination::Exited
        },
    })
}

#[cfg(unix)]
fn status_was_cancelled(status: &ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.code() == Some(130) || status.signal() == Some(2)
}

#[cfg(not(unix))]
fn status_was_cancelled(status: &ExitStatus) -> bool {
    status.code() == Some(130)
}

fn signal_process_tree(parent_pid: u32, signal: &str) {
    let pid = parent_pid.to_string();
    let signal_arg = format!("-{signal}");
    let _ = Command::new("pkill")
        .args([&signal_arg, "-P", &pid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("kill")
        .args([&signal_arg, &pid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn stream_and_capture<R: Read>(
    mut reader: R,
    is_stderr: bool,
    activity: Arc<Mutex<Instant>>,
) -> Result<String> {
    let mut captured = Vec::new();
    let mut buf = [0_u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        *activity.lock() = Instant::now();
        captured.extend_from_slice(&buf[..n]);
        write_console_chunk(&buf[..n], is_stderr)?;
    }
    Ok(decode_command_output(&captured))
}

#[cfg(windows)]
fn write_console_chunk(bytes: &[u8], is_stderr: bool) -> Result<()> {
    let text = decode_command_output(bytes);
    if is_stderr {
        io::stderr().write_all(text.as_bytes())?;
        io::stderr().flush()?;
    } else {
        io::stdout().write_all(text.as_bytes())?;
        io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(windows)]
fn decode_command_output(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let (text, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if !had_errors {
        return text.into_owned();
    }
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(not(windows))]
fn decode_command_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(not(windows))]
fn write_console_chunk(bytes: &[u8], is_stderr: bool) -> Result<()> {
    if is_stderr {
        io::stderr().write_all(bytes)?;
        io::stderr().flush()?;
    } else {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::create_abort_signal;
    use std::time::Duration;

    #[test]
    fn cwd_capture_is_unchanged_without_cwd_file() {
        let shell = Shell::new("sh", "/bin/sh", "-c");
        assert_eq!(with_cwd_capture(&shell, "pwd"), "pwd");
    }

    #[tokio::test]
    async fn controlled_command_cancellation_keeps_partial_output() {
        let shell = Shell::new("sh", "/bin/sh", "-c");
        let abort_signal = create_abort_signal();
        let cancel = abort_signal.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel.set_ctrlc();
        });

        let started = std::time::Instant::now();
        let output =
            run_command_capture_controlled(&shell, "printf before-cancel; sleep 10", abort_signal)
                .await
                .unwrap();

        assert_eq!(output.code, 130);
        assert_eq!(output.termination, CommandTermination::Cancelled);
        assert!(output.stdout.contains("before-cancel"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn sigint_exit_status_is_classified_as_cancelled() {
        let mut child = Command::new("sleep").arg("10").spawn().unwrap();
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap();
        let status = child.wait().unwrap();

        assert!(status_was_cancelled(&status));
    }
}
