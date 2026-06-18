use anyhow::{bail, Context, Result};
use std::{
    io::{self, BufRead, BufReader, Write},
    process::{Command, Stdio},
};

const INSTALL_SH_URL: &str =
    "https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.sh";
const INSTALL_PS1_URL: &str =
    "https://raw.githubusercontent.com/jinzheng8115/aicmd/main/contrib/aicmd/install.ps1";

#[derive(Debug, Default)]
struct UpdateOptions {
    version: Option<String>,
    dry_run: bool,
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

fn installer_command(options: &UpdateOptions) -> InstallerCommand {
    if cfg!(windows) {
        windows_installer_command(options)
    } else {
        posix_installer_command(options)
    }
}

fn posix_installer_command(options: &UpdateOptions) -> InstallerCommand {
    let mut script = format!("curl -fsSL {INSTALL_SH_URL} | bash");
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
            "$p=Join-Path $env:TEMP 'aicmd-install.ps1'; iwr {INSTALL_PS1_URL} -UseBasicParsing -OutFile $p; & $p -Version {}",
            powershell_quote(version)
        )
    } else {
        format!("iwr {INSTALL_PS1_URL} -UseBasicParsing | iex")
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

Update AICmd using the official installer.

用法：aicmd update [--version vX.Y.Z] [--dry-run]

使用官方安装器更新 AICmd。

Options / 参数:
  --version <VERSION>  Install a specific version / 安装指定版本
  --dry-run            Print installer command only / 只输出将执行的安装命令
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
    }

    #[test]
    fn posix_dry_run_command_includes_version() {
        let command = posix_installer_command(&UpdateOptions {
            version: Some("v1.2.3".to_string()),
            dry_run: true,
        });
        assert_eq!(command.program, "sh");
        assert!(command.display().contains("--version v1.2.3"));
    }
}
