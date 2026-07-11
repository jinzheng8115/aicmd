use crate::utils::Shell;

use anyhow::{Context, Result};
use std::{
    env,
    io::{self, Read, Write},
    process::{Command, Stdio},
    thread,
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
}

pub fn run_command_capture(shell: &Shell, command: &str) -> Result<CommandOutput> {
    let mut child = Command::new(&shell.cmd)
        .args([&shell.arg, command])
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("failed to capture stdout")?;
    let stderr = child.stderr.take().context("failed to capture stderr")?;
    let stdout_handle = thread::spawn(move || stream_and_capture(stdout, false));
    let stderr_handle = thread::spawn(move || stream_and_capture(stderr, true));
    let status = child.wait()?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader thread panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;
    Ok(CommandOutput {
        code: status.code().unwrap_or_default(),
        stdout,
        stderr,
    })
}

fn stream_and_capture<R: Read>(mut reader: R, is_stderr: bool) -> Result<String> {
    let mut captured = Vec::new();
    let mut buf = [0_u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
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

    #[test]
    fn cwd_capture_is_unchanged_without_cwd_file() {
        let shell = Shell::new("sh", "/bin/sh", "-c");
        assert_eq!(with_cwd_capture(&shell, "pwd"), "pwd");
    }
}
