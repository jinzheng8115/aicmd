use crate::config::Config;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

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
    let root = mcp_root(&config);
    let servers = root
        .get("servers")
        .or_else(|| root.get("mcpServers"))
        .and_then(Value::as_object)
        .context("MCP config missing mcp.servers")?;
    let commands = mcp_commands(&config)?;
    let command_spec = commands.get(command_name).with_context(|| {
        format!(
            "unknown MCP command: {command_name}\navailable: {}",
            available_commands(commands)
        )
    })?;
    let server_name = command_spec
        .get("server")
        .and_then(Value::as_str)
        .with_context(|| format!("MCP command {command_name:?} requires server"))?;
    let tool_override = command_spec.get("tool").and_then(Value::as_str);
    let server = servers
        .get(server_name)
        .and_then(Value::as_object)
        .with_context(|| {
            format!("MCP server {server_name:?} not found for command {command_name:?}")
        })?;
    if server
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stdio")
        != "stdio"
    {
        bail!("MCP server {server_name:?} type must be stdio");
    }
    let server_command = server
        .get("command")
        .and_then(Value::as_str)
        .with_context(|| format!("MCP server {server_name:?} missing command"))?;
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

    let mut child = spawn_mcp_server(server_command, &server_args, &server_env)
        .with_context(|| format!("failed to start MCP server {server_name:?}: {server_command}"))?;
    let mut stdin = child.stdin.take().context("failed to open MCP stdin")?;
    let stdout = child.stdout.take().context("failed to open MCP stdout")?;
    let stderr = child.stderr.take().context("failed to open MCP stderr")?;
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

    let mut next_id = 1_u64;
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
    read_response(&rx, &mut child, &err_rx, init_id, Duration::from_secs(60))?;
    send_notification(&mut stdin, "notifications/initialized", Some(json!({})))?;

    let selected_tool = if let Some(tool) = tool_override {
        tool.to_string()
    } else {
        let list_id = send_request(&mut stdin, &mut next_id, "tools/list", None)?;
        let tools_result =
            read_response(&rx, &mut child, &err_rx, list_id, Duration::from_secs(60))?;
        choose_tool(
            command_name,
            command_spec,
            user_input,
            server_name,
            tools_result.get("tools"),
        )?
    };
    let tool_arguments = build_tool_arguments(command_spec, &selected_tool, user_input)?;
    let call_id = send_request(
        &mut stdin,
        &mut next_id,
        "tools/call",
        Some(json!({"name": selected_tool, "arguments": tool_arguments})),
    )?;
    let result = read_response(&rx, &mut child, &err_rx, call_id, Duration::from_secs(120))?;
    let _ = child.kill();
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
    timeout: Duration,
) -> Result<Value> {
    loop {
        match rx.recv_timeout(timeout) {
            Ok(msg) => {
                if msg.get("id").and_then(Value::as_u64) != Some(expected_id) {
                    continue;
                }
                if let Some(error) = msg.get("error") {
                    bail!("{}", serde_json::to_string(error)?);
                }
                return Ok(msg.get("result").cloned().unwrap_or_else(|| json!({})));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => bail!("timed out waiting for MCP response"),
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
