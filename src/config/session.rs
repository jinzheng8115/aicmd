use super::input::*;
use super::*;

use crate::client::{Message, MessageContent, MessageRole};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::path::Path;

const MAX_CONTEXT_MESSAGES: usize = 8;
const MAX_CONTEXT_MESSAGE_CHARS: usize = 1200;

fn input_needs_session_context(text: &str) -> bool {
    let text = text
        .split("\n\n参考文件内容 / Reference file contents:")
        .next()
        .unwrap_or(text)
        .to_lowercase();
    let markers = [
        "刚才",
        "上次",
        "上一",
        "之前",
        "继续",
        "基于",
        "根据上",
        "上面的",
        "前面",
        "这个结果",
        "这次错误",
        "报错",
        "修复",
        "previous",
        "previous error",
        "last result",
        "last command",
        "above",
        "continue",
        "this result",
        "that result",
        "fix the previous",
    ];
    markers.iter().any(|marker| text.contains(marker))
}

fn should_include_session_context(context_enabled: bool, text: &str) -> bool {
    context_enabled && input_needs_session_context(text)
}

fn trim_text_for_context(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push_str("\n[context truncated / 上下文已截断]");
    }
    out
}

fn trim_message_for_context(message: &Message) -> Message {
    match &message.content {
        MessageContent::Text(text) if !message.role.is_system() => Message::new(
            message.role,
            MessageContent::Text(trim_text_for_context(text, MAX_CONTEXT_MESSAGE_CHARS)),
        ),
        _ => message.clone(),
    }
}

fn compact_messages_for_context(messages: &[Message]) -> Vec<Message> {
    let other_messages: Vec<Message> = messages
        .iter()
        .filter(|message| !message.role.is_system())
        .map(trim_message_for_context)
        .collect();
    let keep_from = other_messages.len().saturating_sub(MAX_CONTEXT_MESSAGES);
    other_messages.into_iter().skip(keep_from).collect()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Session {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role_name: Option<String>,
    messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    data_urls: HashMap<String, String>,

    #[serde(skip)]
    model: Model,
    #[serde(skip)]
    role_prompt: String,
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    path: Option<String>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    tokens: usize,
    #[serde(skip)]
    context_enabled: bool,
}

impl Session {
    pub fn new(config: &Config, name: &str) -> Self {
        let role = config.extract_role();
        let mut session = Self {
            name: name.to_string(),
            ..Default::default()
        };
        session.set_role(role);
        session.dirty = false;
        session
    }

    pub fn load(config: &Config, name: &str, path: &Path) -> Result<Self> {
        let content = read_to_string(path)
            .with_context(|| format!("Failed to load session {} at {}", name, path.display()))?;
        let mut session: Self =
            serde_yaml::from_str(&content).with_context(|| format!("Invalid session {name}"))?;

        session.model = config.current_model().clone();

        session.name = name.to_string();
        session.path = Some(path.display().to_string());

        if let Some(role_name) = &session.role_name {
            if let Ok(role) = config.retrieve_role(role_name) {
                session.role_prompt = role.prompt().to_string();
            }
        }

        session.update_tokens();

        Ok(session)
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_context_enabled(&mut self, enabled: bool) {
        self.context_enabled = enabled;
    }

    pub fn update_tokens(&mut self) {
        self.tokens = self.model().total_tokens(&self.messages);
    }

    pub fn set_role(&mut self, role: Role) {
        self.temperature = role.temperature();
        self.top_p = role.top_p();
        self.model = role.model().clone();
        self.role_name = convert_option_string(role.name());
        self.role_prompt = role.prompt().to_string();
        self.dirty = true;
        self.update_tokens();
    }

    pub fn save(&mut self, session_name: &str, session_path: &Path) -> Result<()> {
        ensure_parent_exists(session_path)?;

        self.path = Some(session_path.display().to_string());

        let content = serde_yaml::to_string(&self)
            .with_context(|| format!("Failed to serde session '{}'", self.name))?;
        write(session_path, content).with_context(|| {
            format!(
                "Failed to write session '{}' to '{}'",
                self.name,
                session_path.display()
            )
        })?;

        if self.name() != session_name {
            self.name = session_name.to_string()
        }

        self.dirty = false;

        Ok(())
    }

    pub fn persist(&mut self, session_path: &Path) -> Result<()> {
        let session_name = self.name.clone();
        self.save(&session_name, session_path)
    }

    pub fn guard_empty(&self) -> Result<()> {
        if !self.is_empty() {
            bail!("Cannot perform this operation because the session has messages, please `.empty session` first.");
        }
        Ok(())
    }

    pub fn add_message(&mut self, input: &Input, output: &str) -> Result<()> {
        if self.messages.is_empty() {
            self.messages.extend(input.role().build_messages(input));
        } else {
            self.messages
                .push(Message::new(MessageRole::User, input.message_content()));
        }
        self.data_urls.extend(input.data_urls());
        self.messages.push(Message::new(
            MessageRole::Assistant,
            MessageContent::Text(output.to_string()),
        ));
        self.dirty = true;
        self.update_tokens();
        Ok(())
    }

    pub fn add_assistant_note(&mut self, note: String) {
        self.messages.push(Message::new(
            MessageRole::Assistant,
            MessageContent::Text(note),
        ));
        self.dirty = true;
        self.update_tokens();
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.data_urls.clear();
        self.dirty = true;
        self.update_tokens();
    }

    pub fn echo_messages(&self, input: &Input) -> String {
        let messages = self.build_messages(input);
        serde_yaml::to_string(&messages).unwrap_or_else(|_| "Unable to echo message".into())
    }

    pub fn build_messages(&self, input: &Input) -> Vec<Message> {
        if !should_include_session_context(self.context_enabled, &input.text()) {
            return input.role().build_messages(input);
        }

        let mut messages = input.role().build_messages(input);
        let current_user = messages
            .pop()
            .expect("role messages always include the current user input");
        messages.extend(compact_messages_for_context(&self.messages));
        messages.push(current_user);
        messages
    }
}

impl RoleLike for Session {
    fn to_role(&self) -> Role {
        let role_name = self.role_name.as_deref().unwrap_or_default();
        let mut role = Role::new(role_name, &self.role_prompt);
        role.sync(self);
        role
    }

    fn model(&self) -> &Model {
        &self.model
    }

    fn temperature(&self) -> Option<f64> {
        self.temperature
    }

    fn top_p(&self) -> Option<f64> {
        self.top_p
    }

    fn set_model(&mut self, model: Model) {
        if self.model().id() != model.id() {
            self.model = model;
            self.dirty = true;
            self.update_tokens();
        }
    }

    fn set_temperature(&mut self, value: Option<f64>) {
        if self.temperature != value {
            self.temperature = value;
            self.dirty = true;
        }
    }

    fn set_top_p(&mut self, value: Option<f64>) {
        if self.top_p != value {
            self.top_p = value;
            self.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_context_keeps_only_recent_non_system_messages() {
        let mut messages = vec![Message::new(
            MessageRole::System,
            MessageContent::Text("system".to_string()),
        )];
        for i in 0..12 {
            messages.push(Message::new(
                MessageRole::User,
                MessageContent::Text(format!("message-{i}")),
            ));
        }
        let compact = compact_messages_for_context(&messages);
        assert_eq!(compact.len(), 8);
        assert!(compact.iter().all(|message| !message.role.is_system()));
        assert!(compact[0].content.to_text().contains("message-4"));
        assert!(compact[7].content.to_text().contains("message-11"));
    }

    #[test]
    fn current_role_prompt_and_examples_replace_old_session_system_prompt() {
        let config: GlobalConfig = std::sync::Arc::new(parking_lot::RwLock::new(Config::default()));
        let mut session = Session::new(&config.read(), "test");
        session.set_context_enabled(true);
        session.messages = vec![
            Message::new(
                MessageRole::System,
                MessageContent::Text("old session system".to_string()),
            ),
            Message::new(
                MessageRole::User,
                MessageContent::Text("previous user".to_string()),
            ),
            Message::new(
                MessageRole::Assistant,
                MessageContent::Text("previous assistant".to_string()),
            ),
        ];
        let role = Role::new(
            "planner",
            "current role system\n### INPUT:\nexample input\n### OUTPUT:\nexample output",
        );
        let input = Input::from_str(&config, "继续刚才的任务", Some(role));

        let messages = session.build_messages(&input);
        let texts: Vec<String> = messages
            .iter()
            .map(|message| message.content.to_text())
            .collect();

        assert_eq!(texts[0], "current role system");
        assert_eq!(texts[1], "example input");
        assert_eq!(texts[2], "example output");
        assert!(!texts.iter().any(|text| text == "old session system"));
        assert!(texts.iter().any(|text| text == "previous user"));
        assert!(texts.iter().any(|text| text == "previous assistant"));
        assert_eq!(texts.last().unwrap(), "继续刚才的任务");
    }

    #[test]
    fn recent_failure_execution_note_enters_planner_messages() -> Result<()> {
        let mut config = Config::default();
        let mut session = Session::new(&config, "test");
        session.set_context_enabled(true);
        session.messages = vec![
            Message::new(
                MessageRole::System,
                MessageContent::Text("old session system".to_string()),
            ),
            Message::new(
                MessageRole::Assistant,
                MessageContent::Text(
                    "Command execution result:\nCommand:\nfalse\n\nExit code: 1".to_string(),
                ),
            ),
        ];
        config.session = Some(session);
        let config: GlobalConfig = std::sync::Arc::new(parking_lot::RwLock::new(config));
        let planner = Role::builtin(SHELL_ROLE)?;
        let input = Input::from_str(&config, "继续修复刚才失败的任务", None).with_role(planner);

        let messages = input.build_messages()?;
        let request = messages
            .iter()
            .map(|message| message.content.to_text())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(request.contains("Command:\nfalse"));
        assert!(request.contains("Exit code: 1"));
        assert!(!request.contains("old session system"));
        Ok(())
    }

    #[test]
    fn planner_and_command_roles_each_use_their_current_system_prompt() -> Result<()> {
        let config: GlobalConfig = std::sync::Arc::new(parking_lot::RwLock::new(Config::default()));
        let mut session = Session::new(&config.read(), "test");
        session.set_context_enabled(true);
        session.messages = vec![
            Message::new(
                MessageRole::System,
                MessageContent::Text("old session system".to_string()),
            ),
            Message::new(
                MessageRole::Assistant,
                MessageContent::Text("Command:\nfalse\n\nExit code: 1".to_string()),
            ),
        ];
        let cases = [
            (SHELL_ROLE, "execution planner"),
            (SHELL_COMMAND_ROLE, "system operations"),
        ];

        for (role_name, current_prompt_marker) in cases {
            let role = Role::builtin(role_name)?;
            let input = Input::from_str(&config, "继续刚才的任务", Some(role));
            let messages = session.build_messages(&input);
            let request = messages
                .iter()
                .map(|message| message.content.to_text())
                .collect::<Vec<_>>()
                .join("\n");

            assert!(request.contains(current_prompt_marker));
            assert!(request.contains("Command:\nfalse"));
            assert!(!request.contains("old session system"));
        }
        Ok(())
    }

    #[test]
    fn compact_context_truncates_long_text_messages() {
        let messages = vec![Message::new(
            MessageRole::Assistant,
            MessageContent::Text("x".repeat(MAX_CONTEXT_MESSAGE_CHARS + 20)),
        )];
        let compact = compact_messages_for_context(&messages);
        let text = compact[0].content.to_text();
        assert!(text.contains("[context truncated / 上下文已截断]"));
        assert!(text.chars().count() < MAX_CONTEXT_MESSAGE_CHARS + 80);
    }

    #[test]
    fn detects_when_user_needs_session_context() {
        assert!(!input_needs_session_context("目前内存使用率"));
        assert!(!input_needs_session_context(
            "根据参考搜索结果，生成一条命令\n\n参考文件内容 / Reference file contents:\n--- FILE: /tmp/.last.txt ---"
        ));
        assert!(input_needs_session_context("根据刚才的结果继续处理"));
        assert!(input_needs_session_context("fix the previous error"));
    }

    #[test]
    fn context_requires_explicit_session_enablement() {
        let task = "根据刚才的结果继续处理";
        assert!(!should_include_session_context(false, task));
        assert!(should_include_session_context(true, task));
    }

    #[test]
    fn disabled_or_unmarked_context_keeps_role_only_messages() {
        let config: GlobalConfig = std::sync::Arc::new(parking_lot::RwLock::new(Config::default()));
        let role = Role::new("planner", "planner system");
        let cases = [(false, "继续刚才的任务"), (true, "列出当前目录文件")];

        for (context_enabled, text) in cases {
            let mut session = Session::new(&config.read(), "test");
            session.set_context_enabled(context_enabled);
            session.messages.push(Message::new(
                MessageRole::Assistant,
                MessageContent::Text("session history".to_string()),
            ));
            let input = Input::from_str(&config, text, Some(role.clone()));

            let actual = serde_yaml::to_string(&session.build_messages(&input)).unwrap();
            let expected = serde_yaml::to_string(&input.role().build_messages(&input)).unwrap();
            assert_eq!(actual, expected);
        }
    }
}
