use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    cmp::Ordering,
    io::{self, BufRead, BufReader, Write},
    process::{Command, Stdio},
};

const INSTALL_RAW_BASE: &str = "https://raw.githubusercontent.com/jinzheng8115/aicmd";

#[derive(Debug, Default)]
struct UpdateOptions {
    version: Option<String>,
    dry_run: bool,
    check: bool,
}

pub fn run_update_command(args: &[String]) -> Result<i32> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "help" | "-h" | "--help"))
    {
        print_usage();
        return Ok(0);
    }

    let options = parse_args(args)?;
    if options.check {
        return check_latest_version();
    }

    if options.version.is_none() && !options.dry_run && !should_continue_update()? {
        return Ok(0);
    }

    let command = installer_command(&options);
    if options.dry_run {
        println!("{}", command.display());
        return Ok(0);
    }

    eprintln!("Current version: {}", env!("CARGO_PKG_VERSION"));
    eprintln!("Installer command: {}", command.display());
    if !confirm_update()? {
        eprintln!("cancelled");
        return Ok(1);
    }

    let status = command.run()?;
    if status == 0 {
        println!("Update finished. Run: aicmd doctor");
    }
    Ok(status)
}

fn parse_args(args: &[String]) -> Result<UpdateOptions> {
    let mut options = UpdateOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                options.dry_run = true;
                index += 1;
            }
            "--check" => {
                options.check = true;
                index += 1;
            }
            "--version" => {
                let Some(version) = args.get(index + 1) else {
                    bail!("--version requires a value");
                };
                options.version = Some(version.clone());
                index += 2;
            }
            value if value.starts_with("--version=") => {
                options.version = Some(value.trim_start_matches("--version=").to_string());
                index += 1;
            }
            arg => bail!("Unknown option for update: {arg}"),
        }
    }
    if options.check && options.version.is_some() {
        bail!("--check cannot be used with --version");
    }
    if options.check && options.dry_run {
        bail!("--check cannot be used with --dry-run");
    }
    Ok(options)
}

#[derive(Debug)]
struct InstallerCommand {
    program: &'static str,
    args: Vec<String>,
}

impl InstallerCommand {
    fn display(&self) -> String {
        let mut parts = vec![self.program.to_string()];
        parts.extend(self.args.iter().map(|arg| shell_quote(arg)));
        parts.join(" ")
    }

    fn run(&self) -> Result<i32> {
        let status = Command::new(self.program)
            .args(&self.args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("failed to run installer: {}", self.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}

fn check_latest_version() -> Result<i32> {
    let current = current_release_tag();
    let Some(latest) = query_latest_release()? else {
        eprintln!(
            "Could not query latest release. Current version: {} / 无法查询最新版本。当前版本：{}",
            current, current
        );
        return Ok(2);
    };

    println!("Current version: {current}");
    println!("Latest version: {latest}");
    match compare_release_tags(&current, &latest) {
        Ordering::Less => {
            println!("Update available. Run: aicmd update");
            println!("发现新版本。执行：aicmd update");
            Ok(0)
        }
        Ordering::Equal => {
            println!("AICmd is already up to date. / AICmd 已经是最新版本。");
            Ok(0)
        }
        Ordering::Greater => {
            println!(
                "Current version is newer than the latest release. / 当前版本高于最新 Release。"
            );
            Ok(0)
        }
    }
}

fn should_continue_update() -> Result<bool> {
    let current = current_release_tag();
    let latest = match query_latest_release() {
        Ok(Some(latest)) => latest,
        Ok(None) => {
            eprintln!(
                "Warning: could not query latest release before update; continuing with installer fallback."
            );
            eprintln!("警告：更新前无法查询最新版本，将继续使用安装器 fallback。");
            return Ok(true);
        }
        Err(err) => {
            eprintln!("Warning: latest release check failed: {err}; continuing with installer.");
            eprintln!("警告：最新版本检查失败，将继续使用安装器。");
            return Ok(true);
        }
    };

    match compare_release_tags(&current, &latest) {
        Ordering::Less => {
            eprintln!("Current version: {current}");
            eprintln!("Latest version: {latest}");
            Ok(true)
        }
        Ordering::Equal => {
            println!(
                "AICmd is already up to date ({current}). / AICmd 已经是最新版本（{current}）。"
            );
            Ok(false)
        }
        Ordering::Greater => {
            println!(
                "Current version {current} is newer than latest release {latest}. / 当前版本高于最新 Release。"
            );
            Ok(false)
        }
    }
}

fn query_latest_release() -> Result<Option<String>> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "User-Agent: aicmd-updater",
            "https://api.github.com/repos/jinzheng8115/aicmd/releases/latest",
        ])
        .output()
        .context("failed to run curl for latest release check")?;
    if !output.status.success() {
        return Ok(None);
    }
    let json: Value =
        serde_json::from_slice(&output.stdout).context("invalid GitHub release JSON")?;
    Ok(json
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::to_string))
}

fn compare_release_tags(current: &str, latest: &str) -> Ordering {
    parse_release_tag(current).cmp(&parse_release_tag(latest))
}

fn parse_release_tag(tag: &str) -> Vec<u64> {
    tag.trim_start_matches('v')
        .split(['.', '-', '+'])
        .take(3)
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn installer_command(options: &UpdateOptions) -> InstallerCommand {
    if cfg!(windows) {
        windows_installer_command(options)
    } else {
        posix_installer_command(options)
    }
}

fn installer_url(file_name: &str) -> String {
    format!(
        "{}/{}/contrib/aicmd/{}",
        INSTALL_RAW_BASE,
        current_release_tag(),
        file_name
    )
}

fn current_release_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn posix_installer_command(options: &UpdateOptions) -> InstallerCommand {
    let install_url = installer_url("install.sh");
    let mut script = format!("curl -fsSL {install_url} | bash");
    if let Some(version) = &options.version {
        script.push_str(" -s -- --version ");
        script.push_str(&shell_quote(version));
    }
    InstallerCommand {
        program: "sh",
        args: vec!["-c".to_string(), script],
    }
}

fn windows_installer_command(options: &UpdateOptions) -> InstallerCommand {
    let command = if let Some(version) = &options.version {
        format!(
            "$p=Join-Path $env:TEMP 'aicmd-install.ps1'; iwr {} -UseBasicParsing -OutFile $p; & $p -Version {}",
            installer_url("install.ps1"),
            powershell_quote(version)
        )
    } else {
        format!(
            "iwr {} -UseBasicParsing | iex",
            installer_url("install.ps1")
        )
    };
    InstallerCommand {
        program: "powershell",
        args: vec![
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-Command".to_string(),
            command,
        ],
    }
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | ':' | '.' | '_' | '-' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn powershell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn confirm_update() -> Result<bool> {
    let message =
        "Update AICmd by downloading and reinstalling the binary? / 确认下载并重新安装 AICmd？";
    if let Ok(tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    {
        let mut tty_reader = BufReader::new(tty.try_clone()?);
        let mut tty_writer = tty;
        write!(tty_writer, "{message} [y/N] ")?;
        tty_writer.flush()?;
        let mut answer = String::new();
        tty_reader.read_line(&mut answer)?;
        return Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"));
    }

    eprint!("{message} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn print_usage() {
    println!(
        r#"Usage: aicmd update [--version vX.Y.Z] [--dry-run]
       aicmd update --check

Update AICmd using the official installer.

用法：aicmd update [--version vX.Y.Z] [--dry-run]
      aicmd update --check

使用官方安装器更新 AICmd。

Options / 参数:
  --version <VERSION>  Install a specific version / 安装指定版本
  --dry-run            Print installer command only / 只输出将执行的安装命令
  --check              Check latest version only / 只检查最新版本
  -h, --help           Show this help / 显示帮助"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_update_version_equals() {
        let args = vec!["--version=v1.2.3".to_string(), "--dry-run".to_string()];
        let options = parse_args(&args).unwrap();
        assert_eq!(options.version.as_deref(), Some("v1.2.3"));
        assert!(options.dry_run);
        assert!(!options.check);
    }

    #[test]
    fn posix_dry_run_command_includes_version() {
        let command = posix_installer_command(&UpdateOptions {
            version: Some("v1.2.3".to_string()),
            dry_run: true,
            check: false,
        });
        assert_eq!(command.program, "sh");
        assert!(command.display().contains("--version v1.2.3"));
    }

    #[test]
    fn parse_update_check_rejects_version() {
        let args = vec![
            "--check".to_string(),
            "--version".to_string(),
            "v1.2.3".to_string(),
        ];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn dry_run_uses_versioned_installer_url() {
        let command = posix_installer_command(&UpdateOptions {
            version: None,
            dry_run: true,
            check: false,
        });
        let display = command.display();
        assert!(display.contains("/v"));
        assert!(display.contains(env!("CARGO_PKG_VERSION")));
        assert!(!display.contains("/main/contrib/aicmd/install.sh"));
    }

    #[test]
    fn compare_release_versions() {
        assert_eq!(compare_release_tags("v0.30.3", "v0.30.4"), Ordering::Less);
        assert_eq!(compare_release_tags("v0.30.3", "0.30.3"), Ordering::Equal);
        assert_eq!(
            compare_release_tags("v0.31.0", "v0.30.9"),
            Ordering::Greater
        );
    }
}
