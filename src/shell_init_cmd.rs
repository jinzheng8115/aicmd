use anyhow::{bail, Result};

pub fn run_shell_init_command(args: &[String]) -> Result<i32> {
    let shell = args
        .first()
        .map(String::as_str)
        .unwrap_or(default_shell_kind());
    match shell {
        "zsh" | "bash" | "sh" => print_posix_shell(),
        "powershell" | "pwsh" | "ps1" => print_powershell(),
        "help" | "-h" | "--help" => print_usage(),
        _ => bail!("unknown shell for shell-init: {shell}"),
    }
    Ok(0)
}

fn default_shell_kind() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "sh"
    }
}

fn print_usage() {
    println!(
        "Usage: aicmd shell-init [zsh|bash|powershell]\n\nPrint shell integration code so commands like `aicmd 切换到上一层目录` can update the current shell directory after execution."
    );
}

fn print_posix_shell() {
    println!(
        r#"# AICmd shell integration.
# Source this in zsh/bash so commands like `aicmd 切换到上一层目录`
# can update the current shell directory after execution.
aicmd() {{
  local __aicmd_cwd_file __aicmd_status __aicmd_new_cwd
  __aicmd_cwd_file="$(mktemp -t aicmd-cwd.XXXXXX)" || return 1
  AICMD_CWD_FILE="$__aicmd_cwd_file" AICMD_SHELL_INTEGRATION=1 command aicmd "$@"
  __aicmd_status=$?
  if [ -s "$__aicmd_cwd_file" ]; then
    __aicmd_new_cwd="$(cat "$__aicmd_cwd_file")"
    if [ -n "$__aicmd_new_cwd" ] && [ -d "$__aicmd_new_cwd" ] && [ "$PWD" != "$__aicmd_new_cwd" ]; then
      cd "$__aicmd_new_cwd" || __aicmd_status=$?
    fi
  fi
  rm -f "$__aicmd_cwd_file"
  return "$__aicmd_status"
}}"#
    );
}

fn print_powershell() {
    println!(
        r#"# AICmd PowerShell integration.
# Add this to your PowerShell profile so commands like `aicmd 切换到上一层目录`
# can update the current shell directory after execution.
function aicmd {{
  $cwdFile = [System.IO.Path]::GetTempFileName()
  $oldValue = $env:AICMD_CWD_FILE
  $oldIntegration = $env:AICMD_SHELL_INTEGRATION
  $env:AICMD_CWD_FILE = $cwdFile
  try {{
    $exe = (Get-Command aicmd.exe -CommandType Application -ErrorAction SilentlyContinue).Source
    if (-not $exe) {{ $exe = (Get-Command aicmd -CommandType Application).Source }}
    $env:AICMD_SHELL_INTEGRATION = "1"
    & $exe @args
    $status = $LASTEXITCODE
    if ((Test-Path $cwdFile) -and ((Get-Item $cwdFile).Length -gt 0)) {{
      $newCwd = (Get-Content $cwdFile -Raw).Trim()
      if ($newCwd -and (Test-Path $newCwd) -and ((Get-Location).Path -ne $newCwd)) {{
        Set-Location $newCwd
      }}
    }}
  }} finally {{
    if ($null -eq $oldValue) {{ Remove-Item Env:AICMD_CWD_FILE -ErrorAction SilentlyContinue }} else {{ $env:AICMD_CWD_FILE = $oldValue }}
    if ($null -eq $oldIntegration) {{ Remove-Item Env:AICMD_SHELL_INTEGRATION -ErrorAction SilentlyContinue }} else {{ $env:AICMD_SHELL_INTEGRATION = $oldIntegration }}
    Remove-Item $cwdFile -ErrorAction SilentlyContinue
  }}
  if ($status -ne $null) {{ $global:LASTEXITCODE = $status }}
}}"#
    );
}
