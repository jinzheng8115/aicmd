use crate::{config::Config, utils::AbortSignal};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    env, fmt, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct McpDiagnostic {
    pub name: String,
    pub status: &'static str,
    pub detail: String,
    pub suggestion: Option<String>,
}

struct McpChildGuard(Child);

struct McpAttemptControl {
    deadline: Instant,
    timeout: Duration,
    abort_signal: AbortSignal,
}

impl McpAttemptControl {
    fn new(timeout: Duration, abort_signal: AbortSignal) -> Self {
        Self {
            deadline: Instant::now() + timeout,
            timeout,
            abort_signal,
        }
    }
}

impl Drop for McpChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[derive(Debug)]
struct McpResponseTimeout {
    phase: String,
    timeout: Duration,
    stderr: String,
}

impl fmt::Display for McpResponseTimeout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "timed out waiting for MCP response during {} after {}s",
            self.phase,
            self.timeout.as_secs()
        )?;
        if !self.stderr.trim().is_empty() {
            write!(formatter, "\nMCP stderr:\n{}", self.stderr)?;
        }
        Ok(())
    }
}

impl std::error::Error for McpResponseTimeout {}

pub fn run_mcp_command(args: &[String]) -> Result<i32> {
    let command = args.first().map(String::as_str).unwrap_or("help");
    match command {
        "help" | "-h" | "--help" => {
            print_usage();
            Ok(0)
        }
        "list" => {
            let config = load_mcp_config()?;
            let commands = mcp_commands(&config)?;
            for (name, spec) in commands {
                let desc = spec
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if desc.is_empty() {
                    println!("{name}");
                } else {
                    println!("{name}\t{desc}");
                }
            }
            Ok(0)
        }
        _ => {
            let input = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                String::new()
            };
            let output = call_mcp_command(command, &input)?;
            print!("{output}");
            Ok(0)
        }
    }
}

pub fn call_mcp_command(command_name: &str, user_input: &str) -> Result<String> {
    let config = load_mcp_config()?;
    call_mcp_with_config(&config, command_name, user_input, None)
}

pub fn call_mcp_command_controlled(
    command_name: &str,
    user_input: &str,
    timeout: Duration,
    abort_signal: AbortSignal,
) -> Result<String> {
    let config = load_mcp_config()?;
    call_mcp_with_config_controlled(&config, command_name, user_input, timeout, abort_signal)
}

fn call_mcp_with_config_controlled(
    config: &Value,
    command_name: &str,
    user_input: &str,
    timeout: Duration,
    abort_signal: AbortSignal,
) -> Result<String> {
    let control = McpAttemptControl::new(timeout, abort_signal);
    call_mcp_with_config(config, command_name, user_input, Some(&control))
}

fn call_mcp_with_config(
    config: &Value,
    command_name: &str,
    user_input: &str,
    control: Option<&McpAttemptControl>,
) -> Result<String> {
    let root = mcp_root(config);
    let servers = root
        .get("servers")
        .or_else(|| root.get("mcpServers"))
        .and_then(Value::as_object)
        .context("MCP config missing mcp.servers")?;
    let commands = mcp_commands(config)?;
    let command_spec_value = commands.get(command_name).with_context(|| {
        format!(
            "unknown MCP command: {command_name}\navailable: {}",
            available_commands(commands)
        )
    })?;
    let command_spec = command_spec_value
        .as_object()
        .with_context(|| format!("MCP command {command_name:?} configuration must be an object"))?;
    let (server_name, tool_override) = command_mapping(command_name, command_spec)?;
    let server = servers
        .get(server_name)
        .and_then(Value::as_object)
        .with_context(|| {
            format!("MCP server {server_name:?} not found for command {command_name:?}")
        })?;
    let server_command = server_command(server_name, server)?;
    let server_args = server
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let server_env = server
        .get("env")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut child = McpChildGuard(
        spawn_mcp_server(server_command, &server_args, &server_env)
            .map_err(|err| mcp_runtime_error("start", server_name, err, None))?,
    );
    let mut stdin = child
        .0
        .stdin
        .take()
        .context("failed to open MCP stdin")
        .map_err(|err| mcp_runtime_error("start", server_name, err, None))?;
    let stdout = child
        .0
        .stdout
        .take()
        .context("failed to open MCP stdout")
        .map_err(|err| mcp_runtime_error("start", server_name, err, None))?;
    let stderr = child
        .0
        .stderr
        .take()
        .context("failed to open MCP stderr")
        .map_err(|err| mcp_runtime_error("start", server_name, err, None))?;
    let (tx, rx) = mpsc::channel::<Value>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(line.trim()) {
                let _ = tx.send(value);
            }
        }
    });
    let (err_tx, err_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut lines = vec![];
        for line in reader.lines().map_while(Result::ok) {
            lines.push(line);
        }
        let _ = err_tx.send(lines.join("\n"));
    });

    let start_timeout = mcp_timeout("AICMD_MCP_START_TIMEOUT_SECS", 180);
    let call_timeout = mcp_timeout("AICMD_MCP_CALL_TIMEOUT_SECS", 300);
    let mut next_id = 1_u64;
    (|| -> Result<()> {
        let init_id = send_request(
            &mut stdin,
            &mut next_id,
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "aicmd", "version": env!("CARGO_PKG_VERSION")}
            })),
        )?;
        read_response(
            &rx,
            &mut child.0,
            &err_rx,
            init_id,
            "initialize",
            start_timeout,
            control,
        )?;
        send_notification(&mut stdin, "notifications/initialized", Some(json!({})))?;
        Ok(())
    })()
    .map_err(|err| {
        mcp_runtime_error(
            "initialize",
            server_name,
            err,
            Some("AICMD_MCP_START_TIMEOUT_SECS"),
        )
    })?;

    let selected_tool = if let Some(tool) = tool_override {
        tool.to_string()
    } else {
        let tools_result = (|| -> Result<Value> {
            let list_id = send_request(&mut stdin, &mut next_id, "tools/list", None)?;
            read_response(
                &rx,
                &mut child.0,
                &err_rx,
                list_id,
                "tools/list",
                start_timeout,
                control,
            )
        })()
        .map_err(|err| {
            mcp_runtime_error(
                "tools/list",
                server_name,
                err,
                Some("AICMD_MCP_START_TIMEOUT_SECS"),
            )
        })?;
        choose_tool(
            command_name,
            command_spec_value,
            user_input,
            server_name,
            tools_result.get("tools"),
        )
        .map_err(|err| mcp_runtime_error("tool selection", server_name, err, None))?
    };
    let result = (|| -> Result<Value> {
        let tool_arguments = build_tool_arguments(command_spec_value, &selected_tool, user_input)?;
        let call_id = send_request(
            &mut stdin,
            &mut next_id,
            "tools/call",
            Some(json!({"name": selected_tool, "arguments": tool_arguments})),
        )?;
        read_response(
            &rx,
            &mut child.0,
            &err_rx,
            call_id,
            "tools/call",
            call_timeout,
            control,
        )
    })()
    .map_err(|err| {
        mcp_runtime_error(
            "tools/call",
            server_name,
            err,
            Some("AICMD_MCP_CALL_TIMEOUT_SECS"),
        )
    })?;
    Ok(extract_text_content(&result))
}

fn print_usage() {
    println!(
        r#"Usage: aicmd mcp <command> [args...]

Call MCP-backed commands configured in ~/.aicmd/mcp.json.

用法：aicmd mcp <命令> [参数...]

调用 ~/.aicmd/mcp.json 中配置的 MCP 命令。

Commands / 命令:
  list             List configured MCP commands / 列出已配置的 MCP 命令
  <command> ...    Run a configured MCP command / 运行已配置的 MCP 命令
  help             Show help / 显示帮助"#
    );
}

fn mcp_config_path() -> PathBuf {
    env::var("AICMD_MCP_CONFIG_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Config::config_dir().join("mcp.json"))
}

fn load_mcp_config() -> Result<Value> {
    let path = mcp_config_path();
    if !path.exists() {
        bail!(
            "MCP config not found: {}\nCreate it before use, or install from the project mcp.json.",
            path.display()
        );
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read MCP config: {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse MCP config: {}", path.display()))
}

fn mcp_root(config: &Value) -> &Value {
    config.get("mcp").unwrap_or(config)
}

fn mcp_commands(config: &Value) -> Result<&Map<String, Value>> {
    mcp_root(config)
        .get("commands")
        .and_then(Value::as_object)
        .context("MCP config missing mcp.commands")
}

pub fn diagnose_config() -> Vec<McpDiagnostic> {
    let path = mcp_config_path();
    diagnose_path(&path)
}

fn diagnose_path(path: &Path) -> Vec<McpDiagnostic> {
    if !path.exists() {
        return vec![
            diagnostic(
                "MCP config",
                "warning",
                format!("not found at {}", path.display()),
                Some(
                    "Create ~/.aicmd/mcp.json or place mcp.json next to .env and run: aicmd init --from-env --force",
                ),
            ),
            search_not_checked(
                "not checked because MCP config is missing",
                "Configure the search command in ~/.aicmd/mcp.json",
            ),
        ];
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            return vec![
                diagnostic(
                    "MCP config",
                    "error",
                    format!("unable to read {}: {err}", path.display()),
                    Some("Check file permissions for ~/.aicmd/mcp.json"),
                ),
                search_not_checked(
                    "not checked because MCP config cannot be read",
                    "Fix ~/.aicmd/mcp.json, then run: aicmd doctor",
                ),
            ]
        }
    };
    let value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            return vec![
                diagnostic(
                    "MCP config",
                    "error",
                    format!("invalid JSON at {}: {err}", path.display()),
                    Some("Fix ~/.aicmd/mcp.json JSON syntax"),
                ),
                search_not_checked(
                    "not checked because MCP config JSON is invalid",
                    "Fix ~/.aicmd/mcp.json, then run: aicmd doctor",
                ),
            ]
        }
    };

    let mut diagnostics = vec![diagnostic(
        "MCP config",
        "ok",
        path.display().to_string(),
        None,
    )];
    diagnostics.push(search_diagnostic(&value));
    diagnostics.extend(diagnose_value(&value));
    diagnostics
}

fn search_diagnostic(config: &Value) -> McpDiagnostic {
    if mcp_root(config)
        .get("commands")
        .and_then(|commands| commands.get("search"))
        .is_some()
    {
        diagnostic("Search", "ok", "configured", None)
    } else {
        diagnostic(
            "Search",
            "warning",
            "command not configured",
            Some("Add mcp.commands.search to ~/.aicmd/mcp.json"),
        )
    }
}

fn search_not_checked(detail: &str, suggestion: &str) -> McpDiagnostic {
    diagnostic("Search", "warning", detail, Some(suggestion))
}

fn diagnose_value(config: &Value) -> Vec<McpDiagnostic> {
    let root = mcp_root(config);
    let servers = match root
        .get("servers")
        .or_else(|| root.get("mcpServers"))
        .and_then(Value::as_object)
    {
        Some(servers) => servers,
        None => {
            return vec![diagnostic(
                "MCP servers",
                "error",
                "missing mcp.servers",
                Some("Add mcp.servers to ~/.aicmd/mcp.json"),
            )]
        }
    };
    let commands = match mcp_commands(config) {
        Ok(commands) => commands,
        Err(err) => {
            return vec![diagnostic(
                "MCP commands",
                "error",
                err.to_string(),
                Some("Add mcp.commands to ~/.aicmd/mcp.json"),
            )]
        }
    };

    let mut diagnostics = servers
        .iter()
        .map(|(name, server)| diagnose_server(name, server))
        .collect::<Vec<_>>();
    diagnostics.extend(
        commands
            .iter()
            .map(|(name, command)| diagnose_command(name, command, servers)),
    );
    diagnostics
}

fn diagnose_server(name: &str, server: &Value) -> McpDiagnostic {
    let diagnostic_name = format!("MCP server {name}");
    let Some(server) = server.as_object() else {
        return diagnostic(
            diagnostic_name,
            "error",
            "server configuration must be an object",
            Some("Fix this server in ~/.aicmd/mcp.json"),
        );
    };
    let command = match server_command(name, server) {
        Ok(command) => command,
        Err(err) => {
            return diagnostic(
                diagnostic_name,
                "error",
                err.to_string(),
                Some("Fix this server in ~/.aicmd/mcp.json"),
            )
        }
    };
    if !executable_exists(command) {
        let detail = if Path::new(command).components().count() > 1 {
            format!("executable not found: {command}")
        } else {
            format!("executable not found in PATH: {command}")
        };
        return diagnostic(
            diagnostic_name,
            "error",
            detail,
            Some("Install the executable or fix the MCP server command"),
        );
    }

    diagnostic(
        diagnostic_name,
        "ok",
        format!("stdio executable available: {command}"),
        None,
    )
}

fn diagnose_command(name: &str, command: &Value, servers: &Map<String, Value>) -> McpDiagnostic {
    let diagnostic_name = format!("MCP command {name}");
    let Some(command) = command.as_object() else {
        return diagnostic(
            diagnostic_name,
            "error",
            "command configuration must be an object",
            Some("Fix this command in ~/.aicmd/mcp.json"),
        );
    };
    let (server, _) = match command_mapping(name, command) {
        Ok(mapping) => mapping,
        Err(err) => {
            return diagnostic(
                diagnostic_name,
                "error",
                err.to_string(),
                Some("Fix this command in ~/.aicmd/mcp.json"),
            )
        }
    };
    if !servers.contains_key(server) {
        return diagnostic(
            diagnostic_name,
            "error",
            format!("references missing server {server:?}"),
            Some("Add the server or fix this command mapping"),
        );
    }

    diagnostic(
        diagnostic_name,
        "ok",
        format!("mapped to server {server}"),
        None,
    )
}

fn server_command<'a>(name: &str, server: &'a Map<String, Value>) -> Result<&'a str> {
    let subject = format!("MCP server {name:?}");
    match server.get("type") {
        None => {}
        Some(value) => {
            let server_type = value
                .as_str()
                .with_context(|| format!("{subject} type must be a string"))?;
            if server_type != "stdio" {
                bail!("{subject} type must be stdio");
            }
        }
    }
    required_exact_string(server, "command", &subject)
}

fn command_mapping<'a>(
    name: &str,
    command: &'a Map<String, Value>,
) -> Result<(&'a str, Option<&'a str>)> {
    let subject = format!("MCP command {name:?}");
    Ok((
        required_exact_string(command, "server", &subject)?,
        optional_exact_string(command, "tool", &subject)?,
    ))
}

fn required_exact_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    subject: &str,
) -> Result<&'a str> {
    let value = object
        .get(field)
        .with_context(|| format!("{subject} requires a non-empty {field}"))?
        .as_str()
        .with_context(|| format!("{subject} {field} must be a string"))?;
    validate_exact_string(value, field, subject)?;
    Ok(value)
}

fn optional_exact_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    subject: &str,
) -> Result<Option<&'a str>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .with_context(|| format!("{subject} {field} must be a string"))?;
    validate_exact_string(value, field, subject)?;
    Ok(Some(value))
}

fn validate_exact_string(value: &str, field: &str, subject: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{subject} requires a non-empty {field}");
    }
    if value.trim() != value {
        bail!("{subject} {field} must not have leading or trailing whitespace");
    }
    Ok(())
}

fn executable_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return is_executable(path);
    }
    env::var_os("PATH")
        .map(|path| executable_exists_in_path(command, &path))
        .unwrap_or(false)
}

fn executable_exists_in_path(command: &str, path: &std::ffi::OsStr) -> bool {
    env::split_paths(path).any(|dir| is_executable(&dir.join(command)))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn diagnostic(
    name: impl Into<String>,
    status: &'static str,
    detail: impl Into<String>,
    suggestion: Option<&str>,
) -> McpDiagnostic {
    McpDiagnostic {
        name: name.into(),
        status,
        detail: detail.into(),
        suggestion: suggestion.map(str::to_string),
    }
}

pub fn mcp_stage_error(
    stage: &str,
    server: &str,
    detail: anyhow::Error,
    suggestion: &str,
) -> anyhow::Error {
    detail.context(format!(
        "MCP {stage} failed for server {server:?}. {suggestion}"
    ))
}

fn mcp_runtime_error(
    stage: &str,
    server: &str,
    detail: anyhow::Error,
    timeout_env: Option<&str>,
) -> anyhow::Error {
    let suggestion = if detail.downcast_ref::<McpResponseTimeout>().is_some() {
        timeout_env
            .map(|name| format!("Increase {name} and retry"))
            .unwrap_or_else(|| "Run: aicmd doctor".to_string())
    } else {
        "Run: aicmd doctor".to_string()
    };
    mcp_stage_error(stage, server, detail, &suggestion)
}

fn available_commands(commands: &Map<String, Value>) -> String {
    let mut names = commands.keys().cloned().collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        "<none>".into()
    } else {
        names.join(", ")
    }
}

fn env_from_json(values: &Map<String, Value>) -> HashMap<String, String> {
    values
        .iter()
        .filter_map(|(key, value)| {
            let value = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            if value.is_empty() {
                None
            } else {
                Some((key.clone(), value))
            }
        })
        .collect()
}

fn spawn_mcp_server(
    server_command: &str,
    server_args: &[String],
    server_env: &Map<String, Value>,
) -> Result<Child> {
    let envs = env_from_json(server_env);
    let mut command = mcp_process_command(server_command, server_args);
    command
        .envs(envs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn MCP command: {server_command}"))
}

#[cfg(windows)]
fn mcp_process_command(server_command: &str, server_args: &[String]) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(server_command).args(server_args);
    command
}

#[cfg(not(windows))]
fn mcp_process_command(server_command: &str, server_args: &[String]) -> Command {
    let mut command = Command::new(server_command);
    command.args(server_args);
    command
}

fn mcp_timeout(env_name: &str, default_secs: u64) -> Duration {
    let secs = env::var(env_name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_secs);
    Duration::from_secs(secs)
}

fn send_request(
    stdin: &mut ChildStdin,
    next_id: &mut u64,
    method: &str,
    params: Option<Value>,
) -> Result<u64> {
    let id = *next_id;
    *next_id += 1;
    let mut msg = json!({"jsonrpc": "2.0", "id": id, "method": method});
    if let Some(params) = params {
        msg["params"] = params;
    }
    writeln!(stdin, "{}", serde_json::to_string(&msg)?)?;
    stdin.flush()?;
    Ok(id)
}

fn send_notification(stdin: &mut ChildStdin, method: &str, params: Option<Value>) -> Result<()> {
    let mut msg = json!({"jsonrpc": "2.0", "method": method});
    if let Some(params) = params {
        msg["params"] = params;
    }
    writeln!(stdin, "{}", serde_json::to_string(&msg)?)?;
    stdin.flush()?;
    Ok(())
}

fn read_response(
    rx: &mpsc::Receiver<Value>,
    child: &mut Child,
    err_rx: &mpsc::Receiver<String>,
    expected_id: u64,
    phase: &str,
    timeout: Duration,
    control: Option<&McpAttemptControl>,
) -> Result<Value> {
    let local_deadline = Instant::now() + timeout;
    loop {
        if control.is_some_and(|control| control.abort_signal.aborted()) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Aborted.");
        }
        let deadline = control
            .map(|control| control.deadline.min(local_deadline))
            .unwrap_or(local_deadline);
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return mcp_timeout_error(child, err_rx, phase, control.map_or(timeout, |v| v.timeout));
        };
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(msg) => {
                if msg.get("id").and_then(Value::as_u64) != Some(expected_id) {
                    continue;
                }
                if let Some(error) = msg.get("error") {
                    bail!("{}", serde_json::to_string(error)?);
                }
                return Ok(msg.get("result").cloned().unwrap_or_else(|| json!({})));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() < deadline {
                    continue;
                }
                return mcp_timeout_error(
                    child,
                    err_rx,
                    phase,
                    control.map_or(timeout, |v| v.timeout),
                );
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let err = err_rx.try_recv().unwrap_or_default();
                let code = child.try_wait()?.and_then(|s| s.code()).unwrap_or_default();
                if err.trim().is_empty() {
                    bail!("MCP server exited with code {code}");
                }
                bail!("{err}");
            }
        }
    }
}

fn mcp_timeout_error(
    child: &mut Child,
    err_rx: &mpsc::Receiver<String>,
    phase: &str,
    timeout: Duration,
) -> Result<Value> {
    let _ = child.kill();
    let _ = child.wait();
    let stderr = err_rx
        .recv_timeout(Duration::from_millis(250))
        .unwrap_or_default();
    Err(anyhow::Error::new(McpResponseTimeout {
        phase: phase.to_string(),
        timeout,
        stderr,
    }))
}

fn render_template(value: &Value, input: &str) -> Value {
    match value {
        Value::String(s) => Value::String(s.replace("{{input}}", input)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| render_template(item, input))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), render_template(value, input)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn default_arguments(command_spec: &Value, tool: &str, text: &str) -> Value {
    if tool == "tavily_search" || tool == "search" {
        return json!({
            "query": text,
            "topic": "general",
            "search_depth": "advanced",
            "max_results": 5,
            "include_answer": true,
            "include_raw_content": false,
            "include_images": false,
            "include_image_descriptions": false
        });
    }
    if tool == "resolve-library-id" {
        return json!({"query": text, "libraryName": text});
    }
    if let Some(input_field) = command_spec.get("input").and_then(Value::as_str) {
        return json!({input_field: text});
    }
    if text.is_empty() {
        json!({})
    } else {
        json!({"input": text})
    }
}

fn build_tool_arguments(command_spec: &Value, tool: &str, user_input: &str) -> Result<Value> {
    let mut args = if let Some(arguments) = command_spec.get("arguments") {
        let rendered = render_template(arguments, user_input);
        if !rendered.is_object() {
            bail!("MCP command arguments must be an object");
        }
        rendered
    } else {
        default_arguments(command_spec, tool, user_input)
    };
    if let Some(options) = command_spec.get("options") {
        let rendered = render_template(options, user_input);
        let Some(options_map) = rendered.as_object() else {
            bail!("MCP command options must be an object");
        };
        let map = args
            .as_object_mut()
            .context("MCP command arguments must be an object")?;
        for (key, value) in options_map {
            map.insert(key.clone(), value.clone());
        }
    }
    Ok(args)
}

fn tokenize(value: &str) -> Vec<String> {
    let mut out = vec![];
    let mut current = String::new();
    for ch in value.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn choose_tool(
    command_name: &str,
    command_spec: &Value,
    user_input: &str,
    server_name: &str,
    tools_value: Option<&Value>,
) -> Result<String> {
    let tools = tools_value
        .and_then(Value::as_array)
        .context("MCP tools/list returned no tools")?;
    if tools.is_empty() {
        bail!("MCP server {server_name:?} exposes no tools");
    }
    if tools.len() == 1 {
        if let Some(name) = tools[0].get("name").and_then(Value::as_str) {
            return Ok(name.to_string());
        }
    }
    let desired = format!(
        "{} {} {}",
        command_name,
        command_spec
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(""),
        user_input
    );
    let desired_tokens = tokenize(&desired);
    let normalized_command = tokenize(command_name).join("");
    let mut scored = vec![];
    for tool in tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        let desc = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let normalized_name = tokenize(name).join("");
        let tool_text = format!("{name} {desc}");
        let tool_tokens = tokenize(&tool_text);
        let mut score = desired_tokens
            .iter()
            .filter(|token| tool_tokens.contains(token))
            .count() as i32;
        if normalized_name == normalized_command {
            score += 100;
        }
        if !normalized_command.is_empty() && normalized_name.contains(&normalized_command) {
            score += 20;
        }
        if !normalized_name.is_empty() && normalized_command.contains(&normalized_name) {
            score += 10;
        }
        scored.push((score, name.to_string(), desc.to_string()));
    }
    scored.sort_by_key(|item| std::cmp::Reverse(item.0));
    if let Some(best) = scored.first() {
        if best.0 > 0 && (scored.len() == 1 || best.0 > scored[1].0) {
            return Ok(best.1.clone());
        }
    }
    let mut lines = vec![
        format!("Cannot automatically choose a tool for MCP command {command_name:?} on server {server_name:?}."),
        "Available tools:".to_string(),
    ];
    for (_, name, desc) in scored {
        if desc.is_empty() {
            lines.push(format!("- {name}"));
        } else {
            lines.push(format!("- {name}: {desc}"));
        }
    }
    lines.push("Add an optional tool field to this command if needed.".into());
    bail!("{}", lines.join("\n"));
}

fn extract_text_content(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{run_external_with_managed_retry, ProgressStage, RetryBudget, RetryPolicy};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn diagnostic_for<'a>(diagnostics: &'a [McpDiagnostic], name: &str) -> &'a McpDiagnostic {
        diagnostics
            .iter()
            .find(|diagnostic| diagnostic.name == name)
            .unwrap_or_else(|| panic!("missing diagnostic {name:?}: {diagnostics:#?}"))
    }

    #[test]
    fn diagnose_rejects_command_with_missing_server() {
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {},
                "commands": {"search": {"server": "missing"}}
            }
        }));

        let diagnostic = diagnostic_for(&diagnostics, "MCP command search");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("missing"));
    }

    #[test]
    fn diagnose_rejects_unsupported_server_type() {
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {
                    "web": {"type": "http", "command": "/bin/echo"}
                },
                "commands": {"search": {"server": "web"}}
            }
        }));

        let diagnostic = diagnostic_for(&diagnostics, "MCP server web");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("stdio"));
    }

    #[test]
    fn diagnose_rejects_missing_or_empty_server_command() {
        for server in [
            json!({"type": "stdio"}),
            json!({"type": "stdio", "command": ""}),
        ] {
            let diagnostics = diagnose_value(&json!({
                "mcp": {
                    "servers": {"web": server},
                    "commands": {"search": {"server": "web"}}
                }
            }));

            let diagnostic = diagnostic_for(&diagnostics, "MCP server web");
            assert_eq!(diagnostic.status, "error");
            assert!(diagnostic.detail.contains("non-empty command"));
        }
    }

    #[test]
    fn diagnose_rejects_missing_absolute_executable_without_leaking_env() {
        let secret = "super-secret-value";
        let missing = env::temp_dir().join("aicmd-missing-mcp-executable");
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {
                    "web": {
                        "type": "stdio",
                        "command": missing,
                        "env": {"API_KEY": secret}
                    }
                },
                "commands": {"search": {"server": "web"}}
            }
        }));

        let diagnostic = diagnostic_for(&diagnostics, "MCP server web");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("not found"));
        assert!(!format!("{diagnostics:#?}").contains(secret));
    }

    #[test]
    fn diagnose_rejects_missing_path_executable() {
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {
                    "web": {
                        "type": "stdio",
                        "command": "aicmd-command-that-does-not-exist-4f8b09"
                    }
                },
                "commands": {"search": {"server": "web"}}
            }
        }));

        let diagnostic = diagnostic_for(&diagnostics, "MCP server web");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("PATH"));
    }

    #[cfg(unix)]
    #[test]
    fn executable_exists_requires_unix_execute_permission() {
        let path = env::temp_dir().join(format!(
            "aicmd-executable-permission-{}",
            std::process::id()
        ));
        fs::write(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!executable_exists(path.to_str().unwrap()));

        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(executable_exists(path.to_str().unwrap()));
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn executable_exists_checks_path_candidate_permission() {
        let dir = env::temp_dir().join(format!("aicmd-path-permission-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let command = dir.join("fake-mcp-command");
        fs::write(&command, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!executable_exists_in_path(
            "fake-mcp-command",
            dir.as_os_str()
        ));

        fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(executable_exists_in_path(
            "fake-mcp-command",
            dir.as_os_str()
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diagnose_rejects_empty_optional_tool() {
        let executable = env::current_exe().unwrap();
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {
                    "web": {"type": "stdio", "command": executable}
                },
                "commands": {"search": {"server": "web", "tool": " "}}
            }
        }));

        let diagnostic = diagnostic_for(&diagnostics, "MCP command search");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("tool"));
    }

    #[test]
    fn doctor_and_runtime_reject_edge_whitespace_consistently() {
        let executable = env::current_exe().unwrap();
        let cases = [
            (
                "MCP server web",
                json!({
                    "mcp": {
                        "servers": {
                            "web": {"command": format!(" {}", executable.display())}
                        },
                        "commands": {"search": {"server": "web", "tool": "search"}}
                    }
                }),
            ),
            (
                "MCP command search",
                json!({
                    "mcp": {
                        "servers": {"web": {"command": executable}},
                        "commands": {"search": {"server": "web ", "tool": "search"}}
                    }
                }),
            ),
            (
                "MCP command search",
                json!({
                    "mcp": {
                        "servers": {"web": {"command": executable}},
                        "commands": {"search": {"server": "web", "tool": " search"}}
                    }
                }),
            ),
        ];

        for (diagnostic_name, config) in cases {
            let diagnostics = diagnose_value(&config);
            let diagnostic = diagnostic_for(&diagnostics, diagnostic_name);
            assert_eq!(diagnostic.status, "error");
            assert!(diagnostic.detail.contains("leading or trailing whitespace"));

            let error = call_mcp_with_config(&config, "search", "query", None).unwrap_err();
            assert!(error.to_string().contains("leading or trailing whitespace"));
        }
    }

    #[test]
    fn missing_type_defaults_to_stdio_but_non_string_type_is_rejected() {
        let executable = env::current_exe().unwrap();
        let invalid = json!({
            "mcp": {
                "servers": {"web": {"type": 1, "command": executable}},
                "commands": {"search": {"server": "web", "tool": "search"}}
            }
        });

        let diagnostics = diagnose_value(&invalid);
        let diagnostic = diagnostic_for(&diagnostics, "MCP server web");
        assert_eq!(diagnostic.status, "error");
        assert!(diagnostic.detail.contains("type must be a string"));

        let error = call_mcp_with_config(&invalid, "search", "query", None).unwrap_err();
        assert!(error.to_string().contains("type must be a string"));
    }

    #[test]
    fn diagnose_accepts_valid_server_and_command_mapping() {
        let executable = env::current_exe().unwrap();
        let diagnostics = diagnose_value(&json!({
            "mcp": {
                "servers": {
                    "web": {"type": "stdio", "command": executable}
                },
                "commands": {
                    "search": {"server": "web", "tool": "search"}
                }
            }
        }));

        assert_eq!(diagnostic_for(&diagnostics, "MCP server web").status, "ok");
        assert_eq!(
            diagnostic_for(&diagnostics, "MCP command search").status,
            "ok"
        );
    }

    #[test]
    fn diagnose_path_reports_invalid_json() {
        let path = env::temp_dir().join(format!(
            "aicmd-invalid-mcp-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::write(&path, "{invalid").unwrap();

        let diagnostics = diagnose_path(&path);

        fs::remove_file(&path).unwrap();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].status, "error");
        assert!(diagnostics[0].detail.contains("invalid JSON"));
        assert_eq!(diagnostics[1].name, "Search");
        assert!(diagnostics[1].detail.contains("not checked"));
    }

    #[test]
    fn search_diagnostic_keeps_configured_missing_and_not_checked_states() {
        let configured = search_diagnostic(&json!({
            "mcp": {
                "servers": {},
                "commands": {"search": {"server": "web"}}
            }
        }));
        assert_eq!(configured.status, "ok");
        assert_eq!(configured.detail, "configured");

        let missing = search_diagnostic(&json!({
            "mcp": {"servers": {}, "commands": {}}
        }));
        assert_eq!(missing.status, "warning");
        assert_eq!(missing.detail, "command not configured");

        let path = env::temp_dir().join(format!(
            "aicmd-missing-mcp-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let diagnostics = diagnose_path(&path);
        let not_checked = diagnostic_for(&diagnostics, "Search");
        assert_eq!(not_checked.status, "warning");
        assert_eq!(
            not_checked.detail,
            "not checked because MCP config is missing"
        );
    }

    #[test]
    fn stage_error_labels_stage_and_preserves_source() {
        let error = mcp_stage_error(
            "initialize",
            "web",
            anyhow::anyhow!("original failure"),
            "Run: aicmd doctor",
        );

        assert!(error.to_string().contains("initialize"));
        assert!(error.to_string().contains("web"));
        assert!(error.to_string().contains("aicmd doctor"));
        assert_eq!(error.source().unwrap().to_string(), "original failure");
    }

    #[test]
    fn stage_error_supports_each_runtime_stage() {
        for stage in [
            "start",
            "initialize",
            "tools/list",
            "tool selection",
            "tools/call",
        ] {
            let error = mcp_stage_error(
                stage,
                "web",
                anyhow::anyhow!("original failure"),
                "Run: aicmd doctor",
            );
            assert!(error.to_string().contains(stage));
        }
    }

    #[test]
    fn only_structured_receive_timeouts_name_the_timeout_variable() {
        let timeout = anyhow::Error::new(McpResponseTimeout {
            phase: "tools/call".into(),
            timeout: Duration::from_secs(1),
            stderr: String::new(),
        });
        let error = mcp_runtime_error(
            "tools/call",
            "web",
            timeout,
            Some("AICMD_MCP_CALL_TIMEOUT_SECS"),
        );
        assert!(error.to_string().contains("AICMD_MCP_CALL_TIMEOUT_SECS"));

        let rpc_error = mcp_runtime_error(
            "tools/call",
            "web",
            anyhow::Error::msg(r#"{"message":"upstream timed out"}"#),
            Some("AICMD_MCP_CALL_TIMEOUT_SECS"),
        );
        assert!(rpc_error.to_string().contains("aicmd doctor"));
        assert!(!rpc_error
            .to_string()
            .contains("AICMD_MCP_CALL_TIMEOUT_SECS"));
    }

    #[cfg(unix)]
    #[test]
    fn mcp_child_is_reaped_after_success_and_errors() {
        for mode in ["success", "initialize_error", "rpc_error"] {
            let pid_file = env::temp_dir().join(format!(
                "aicmd-fake-mcp-{mode}-{}-{}.pid",
                std::process::id(),
                std::thread::current().name().unwrap_or("test")
            ));
            let secret = "fake-mcp-env-secret";
            let config = fake_mcp_config(mode, &pid_file, secret);

            let result = call_mcp_with_config(&config, "search", "query", None);
            let pid = fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap();
            fs::remove_file(&pid_file).unwrap();

            if mode == "success" {
                assert_eq!(result.unwrap(), "fake result");
            } else {
                let error = result.unwrap_err();
                let display = format!("{error:#}");
                assert!(display.contains("aicmd doctor"));
                assert!(!display.contains("AICMD_MCP_CALL_TIMEOUT_SECS"));
                assert!(!display.contains(secret));
            }
            assert!(!process_exists(pid), "fake MCP child {pid} was not reaped");
        }
    }

    #[cfg(unix)]
    #[test]
    fn mcp_timeout_preserves_stderr_and_timeout_advice() {
        let pid_file =
            env::temp_dir().join(format!("aicmd-fake-mcp-timeout-{}.pid", std::process::id()));
        let config = fake_mcp_config("timeout_with_stderr", &pid_file, "unused");
        env::set_var("AICMD_MCP_START_TIMEOUT_SECS", "1");

        let error = call_mcp_with_config(&config, "search", "query", None).unwrap_err();
        env::remove_var("AICMD_MCP_START_TIMEOUT_SECS");
        let display = format!("{error:#}");
        let pid = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        fs::remove_file(pid_file).unwrap();

        assert!(display.contains("fatal: missing FAKE_API_KEY"));
        assert!(display.contains("AICMD_MCP_START_TIMEOUT_SECS"));
        assert!(!process_exists(pid), "fake MCP child {pid} was not reaped");
    }

    #[cfg(unix)]
    #[test]
    fn controlled_mcp_attempt_uses_shared_short_deadline() {
        let pid_file = env::temp_dir().join(format!(
            "aicmd-fake-mcp-controlled-timeout-{}.pid",
            std::process::id()
        ));
        let config = fake_mcp_config("timeout_with_stderr", &pid_file, "unused");
        let started = std::time::Instant::now();

        let error = call_mcp_with_config_controlled(
            &config,
            "search",
            "query",
            Duration::from_millis(100),
            crate::utils::create_abort_signal(),
        )
        .unwrap_err();
        let display = format!("{error:#}");
        let pid = fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse::<u32>()
            .unwrap();
        fs::remove_file(pid_file).unwrap();

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(display.contains("fatal: missing FAKE_API_KEY"));
        assert!(!process_exists(pid), "fake MCP child {pid} was not reaped");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn controlled_mcp_restarts_server_until_third_attempt_succeeds() {
        let pid_file =
            env::temp_dir().join(format!("aicmd-fake-mcp-retry-{}.pids", std::process::id()));
        let attempt_file = env::temp_dir().join(format!(
            "aicmd-fake-mcp-retry-{}.attempts",
            std::process::id()
        ));
        let mut config = fake_mcp_config("timeout_twice_then_success", &pid_file, "unused");
        config["mcp"]["servers"]["fake"]["env"]["AICMD_TEST_FAKE_MCP_ATTEMPT_FILE"] =
            attempt_file.display().to_string().into();
        let budget = RetryBudget::with_policy(RetryPolicy {
            attempt_timeout: Duration::from_millis(150),
            total_timeout: Duration::from_secs(3),
            max_attempts: 3,
        });
        let abort_signal = crate::utils::create_abort_signal();

        let result = run_external_with_managed_retry(
            ProgressStage::new("正在调用测试 MCP", "Calling test MCP"),
            &budget,
            abort_signal.clone(),
            |attempt| {
                let config = config.clone();
                let abort_signal = abort_signal.clone();
                async move {
                    tokio::task::spawn_blocking(move || {
                        call_mcp_with_config_controlled(
                            &config,
                            "search",
                            "query",
                            attempt.timeout,
                            abort_signal,
                        )
                    })
                    .await
                    .unwrap()
                }
            },
        )
        .await
        .unwrap();

        let pids = fs::read_to_string(&pid_file).unwrap();
        fs::remove_file(pid_file).unwrap();
        fs::remove_file(attempt_file).unwrap();
        assert_eq!(result, "fake result");
        assert_eq!(pids.lines().count(), 3);
        for pid in pids.lines().map(|value| value.parse::<u32>().unwrap()) {
            assert!(!process_exists(pid), "fake MCP child {pid} was not reaped");
        }
    }

    #[test]
    fn fake_mcp_server_child() {
        let Ok(mode) = env::var("AICMD_TEST_FAKE_MCP_MODE") else {
            return;
        };
        let pid_file = env::var("AICMD_TEST_FAKE_MCP_PID_FILE").unwrap();
        if mode == "timeout_twice_then_success" {
            use std::fs::OpenOptions;
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(pid_file)
                .unwrap();
            writeln!(file, "{}", std::process::id()).unwrap();
        } else {
            fs::write(pid_file, std::process::id().to_string()).unwrap();
        }
        let attempt = env::var("AICMD_TEST_FAKE_MCP_ATTEMPT_FILE")
            .ok()
            .map(|path| {
                let value = fs::read_to_string(&path)
                    .ok()
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0)
                    + 1;
                fs::write(path, value.to_string()).unwrap();
                value
            })
            .unwrap_or(1);
        if mode == "timeout_twice_then_success" && attempt < 3 {
            eprintln!("temporary MCP timeout on attempt {attempt}");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
        if mode == "timeout_with_stderr" {
            eprintln!("fatal: missing FAKE_API_KEY");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines().map_while(Result::ok) {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let Some(id) = message.get("id").and_then(Value::as_u64) else {
                continue;
            };
            let response = match message.get("method").and_then(Value::as_str) {
                Some("initialize") if mode == "initialize_error" => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32000, "message": "initialize failed"}
                }),
                Some("initialize") => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
                Some("tools/call") if mode == "rpc_error" => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32000, "message": "upstream timed out"}
                }),
                Some("tools/call") => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {"content": [{"type": "text", "text": "fake result"}]}
                }),
                _ => continue,
            };
            writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
            stdout.flush().unwrap();
            if mode == "initialize_error"
                || message.get("method").and_then(Value::as_str) == Some("tools/call")
            {
                loop {
                    thread::sleep(Duration::from_secs(60));
                }
            }
        }
    }

    #[cfg(unix)]
    fn fake_mcp_config(mode: &str, pid_file: &Path, secret: &str) -> Value {
        json!({
            "mcp": {
                "servers": {
                    "fake": {
                        "command": env::current_exe().unwrap(),
                        "args": [
                            "--exact",
                            "mcp_cmd::tests::fake_mcp_server_child",
                            "--nocapture"
                        ],
                        "env": {
                            "AICMD_TEST_FAKE_MCP_MODE": mode,
                            "AICMD_TEST_FAKE_MCP_PID_FILE": pid_file,
                            "AICMD_TEST_FAKE_MCP_SECRET": secret
                        }
                    }
                },
                "commands": {
                    "search": {"server": "fake", "tool": "search"}
                }
            }
        })
    }

    #[cfg(unix)]
    fn process_exists(pid: u32) -> bool {
        extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        unsafe { kill(pid as i32, 0) == 0 }
    }
}
