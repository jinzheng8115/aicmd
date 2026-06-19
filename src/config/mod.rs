mod input;
mod role;
mod session;

pub use self::input::Input;
pub use self::role::{
    Role, RoleLike, COMMAND_SUMMARY_ROLE, EXPLAIN_SHELL_ROLE, MCP_SUMMARY_ROLE, SHELL_ROLE,
};
use self::session::Session;

use crate::client::{list_models, ClientConfig, Model, ModelType, OPENAI_COMPATIBLE_PROVIDERS};
use crate::render::{MarkdownRender, RenderOptions};
use crate::utils::*;

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::json;
use simplelog::LevelFilter;
use std::collections::HashMap;
use std::{
    env,
    fs::{create_dir_all, read_to_string, remove_file},
    path::{Path, PathBuf},
    sync::Arc,
};
use syntect::highlighting::ThemeSet;
use terminal_colorsaurus::{color_scheme, ColorScheme, QueryOptions};

pub const TEMP_SESSION_NAME: &str = "temp";

/// Monokai Extended
const DARK_THEME: &[u8] = include_bytes!("../../assets/monokai-extended.theme.bin");
const LIGHT_THEME: &[u8] = include_bytes!("../../assets/monokai-extended-light.theme.bin");

const CONFIG_FILE_NAME: &str = "config.yaml";
const ENV_FILE_NAME: &str = ".env";
const SESSIONS_DIR_NAME: &str = "sessions";
const CLIENTS_FIELD: &str = "clients";

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename(serialize = "model", deserialize = "model"))]
    #[serde(default)]
    pub model_id: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,

    pub dry_run: bool,
    #[serde(skip)]
    pub print_command: bool,
    pub ai_summary: bool,
    pub stream: bool,
    pub wrap: Option<String>,
    pub wrap_code: bool,

    #[serde(default)]
    pub document_loaders: HashMap<String, String>,

    pub highlight: bool,
    pub theme: Option<String>,

    pub user_agent: Option<String>,
    pub save_shell_history: bool,

    pub clients: Vec<ClientConfig>,

    #[serde(skip)]
    pub info_flag: bool,
    #[serde(skip)]
    pub model: Model,
    #[serde(skip)]
    pub last_message: Option<LastMessage>,

    #[serde(skip)]
    pub role: Option<Role>,
    #[serde(skip)]
    pub session: Option<Session>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model_id: Default::default(),
            temperature: None,
            top_p: None,

            dry_run: false,
            print_command: false,
            ai_summary: true,
            stream: true,
            wrap: None,
            wrap_code: false,

            document_loaders: Default::default(),

            highlight: true,
            theme: None,

            user_agent: None,
            save_shell_history: true,

            clients: vec![],

            info_flag: false,
            model: Default::default(),
            last_message: None,

            role: None,
            session: None,
        }
    }
}

pub type GlobalConfig = Arc<RwLock<Config>>;

impl Config {
    pub async fn init(info_flag: bool) -> Result<Self> {
        Self::migrate_legacy_aichat_config()?;
        let config_path = Self::config_file();
        let mut config = if !config_path.exists() {
            match env::var(get_env_name("provider"))
                .ok()
                .or_else(|| env::var(get_env_name("platform")).ok())
            {
                Some(v) => Self::load_dynamic(&v)?,
                None => bail!("{}", missing_config_guidance(&config_path)),
            }
        } else {
            Self::load_from_file(&config_path)?
        };

        config.info_flag = info_flag;

        let setup = |config: &mut Self| -> Result<()> {
            config.load_envs();

            if let Some(wrap) = config.wrap.clone() {
                config.set_wrap(&wrap)?;
            }

            config.setup_model()?;
            config.setup_document_loaders();
            config.setup_user_agent();
            Ok(())
        };
        let ret = setup(&mut config);
        if !info_flag {
            ret?;
        }
        Ok(config)
    }

    pub fn config_dir() -> PathBuf {
        if let Ok(v) = env::var(get_env_name("config_dir")) {
            PathBuf::from(v)
        } else {
            home_aicmd_dir()
        }
    }

    pub fn local_path(name: &str) -> PathBuf {
        Self::config_dir().join(name)
    }

    pub fn config_file() -> PathBuf {
        match env::var(get_env_name("config_file")) {
            Ok(value) => PathBuf::from(value),
            Err(_) => Self::local_path(CONFIG_FILE_NAME),
        }
    }

    pub fn env_file() -> PathBuf {
        match env::var(get_env_name("env_file")) {
            Ok(value) => PathBuf::from(value),
            Err(_) => Self::local_path(ENV_FILE_NAME),
        }
    }

    pub fn sessions_dir(&self) -> PathBuf {
        match env::var(get_env_name("sessions_dir")) {
            Ok(value) => PathBuf::from(value),
            Err(_) => Self::local_path(SESSIONS_DIR_NAME),
        }
    }

    pub fn session_file(&self, name: &str) -> PathBuf {
        match name.split_once("/") {
            Some((dir, name)) => self.sessions_dir().join(dir).join(format!("{name}.yaml")),
            None => self.sessions_dir().join(format!("{name}.yaml")),
        }
    }

    fn legacy_config_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![];
        if let Ok(v) = env::var("XDG_CONFIG_HOME") {
            dirs.push(PathBuf::from(v).join(env!("CARGO_CRATE_NAME")));
            dirs.push(PathBuf::from(env::var("XDG_CONFIG_HOME").unwrap()).join("aichat"));
        }
        if let Some(dir) = dirs::config_dir() {
            dirs.push(dir.join(env!("CARGO_CRATE_NAME")));
            dirs.push(dir.join("aichat"));
        }
        dirs
    }

    fn migrate_legacy_aichat_config() -> Result<()> {
        if env::var(get_env_name("config_dir")).is_ok()
            || env::var(get_env_name("config_file")).is_ok()
        {
            return Ok(());
        }

        let migrations = [(CONFIG_FILE_NAME, 0o600), (ENV_FILE_NAME, 0o600)];
        for legacy_dir in Self::legacy_config_dirs() {
            if legacy_dir == Self::config_dir() {
                continue;
            }
            for (name, mode) in migrations {
                #[cfg(not(unix))]
                let _ = mode;
                let target = Self::local_path(name);
                let source = legacy_dir.join(name);
                if target.exists() || !source.exists() {
                    continue;
                }
                ensure_parent_exists(&target)?;
                std::fs::copy(&source, &target).with_context(|| {
                    format!(
                        "Failed to migrate legacy config '{}' to '{}'",
                        source.display(),
                        target.display()
                    )
                })?;
                #[cfg(unix)]
                {
                    use std::os::unix::prelude::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&target, perms)?;
                }
            }
        }
        Ok(())
    }
    pub fn log_config() -> Result<(LevelFilter, Option<PathBuf>)> {
        let log_level = env::var(get_env_name("log_level"))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(match cfg!(debug_assertions) {
                true => LevelFilter::Debug,
                false => LevelFilter::Off,
            });
        if log_level == LevelFilter::Off {
            return Ok((log_level, None));
        }
        let log_path = match env::var(get_env_name("log_path")) {
            Ok(v) => Some(PathBuf::from(v)),
            Err(_) => Some(Config::local_path(&format!(
                "{}.log",
                env!("CARGO_CRATE_NAME")
            ))),
        };
        Ok((log_level, log_path))
    }

    pub fn current_model(&self) -> &Model {
        if let Some(session) = self.session.as_ref() {
            session.model()
        } else if let Some(role) = self.role.as_ref() {
            role.model()
        } else {
            &self.model
        }
    }

    pub fn role_like_mut(&mut self) -> Option<&mut dyn RoleLike> {
        if let Some(session) = self.session.as_mut() {
            Some(session)
        } else if let Some(role) = self.role.as_mut() {
            Some(role)
        } else {
            None
        }
    }

    pub fn extract_role(&self) -> Role {
        if let Some(session) = self.session.as_ref() {
            session.to_role()
        } else if let Some(role) = self.role.as_ref() {
            role.clone()
        } else {
            let mut role = Role::default();
            role.batch_set(&self.model, self.temperature, self.top_p);
            role
        }
    }

    pub fn set_wrap(&mut self, value: &str) -> Result<()> {
        if value == "no" {
            self.wrap = None;
        } else if value == "auto" {
            self.wrap = Some(value.into());
        } else {
            value
                .parse::<u16>()
                .map_err(|_| anyhow!("Invalid wrap value"))?;
            self.wrap = Some(value.into())
        }
        Ok(())
    }
    pub fn set_model(&mut self, model_id: &str) -> Result<()> {
        let model = Model::retrieve_model(self, model_id, ModelType::Chat)?;
        match self.role_like_mut() {
            Some(role_like) => role_like.set_model(model),
            None => {
                self.model = model;
            }
        }
        Ok(())
    }

    pub fn use_role(&mut self, name: &str) -> Result<()> {
        let role = self.retrieve_role(name)?;
        self.use_role_obj(role)
    }

    pub fn use_role_obj(&mut self, role: Role) -> Result<()> {
        if let Some(session) = self.session.as_mut() {
            session.guard_empty()?;
            session.set_role(role);
        } else {
            self.role = Some(role);
        }
        Ok(())
    }
    pub fn retrieve_role(&self, name: &str) -> Result<Role> {
        let mut role = Role::builtin(name)?;
        let current_model = self.current_model().clone();
        match role.model_id() {
            Some(model_id) => {
                if current_model.id() != model_id {
                    let model = Model::retrieve_model(self, model_id, ModelType::Chat)?;
                    role.set_model(model);
                } else {
                    role.set_model(current_model);
                }
            }
            None => {
                role.set_model(current_model);
                if role.temperature().is_none() {
                    role.set_temperature(self.temperature);
                }
                if role.top_p().is_none() {
                    role.set_top_p(self.top_p);
                }
            }
        }
        Ok(role)
    }
    pub fn use_session(&mut self, session_name: Option<&str>) -> Result<()> {
        if self.session.is_some() {
            bail!(
                "Already in a session, please run '.exit session' first to exit the current session."
            );
        }
        let session;
        match session_name {
            None | Some(TEMP_SESSION_NAME) => {
                let session_file = self.session_file(TEMP_SESSION_NAME);
                if session_file.exists() {
                    remove_file(session_file).with_context(|| {
                        format!("Failed to cleanup previous '{TEMP_SESSION_NAME}' session")
                    })?;
                }
                session = Some(Session::new(self, TEMP_SESSION_NAME));
            }
            Some(name) => {
                let session_path = self.session_file(name);
                if !session_path.exists() {
                    session = Some(Session::new(self, name));
                } else {
                    session = Some(Session::load(self, name, &session_path)?);
                }
            }
        }
        self.session = session;
        Ok(())
    }
    pub fn empty_session(&mut self) -> Result<()> {
        if let Some(session) = self.session.as_mut() {
            session.clear_messages();
        } else {
            bail!("No session")
        }
        self.discontinuous_last_message();
        Ok(())
    }

    pub fn list_sessions(&self) -> Vec<String> {
        list_file_names(self.sessions_dir(), ".yaml")
    }
    pub fn save_current_session(&mut self) -> Result<Option<String>> {
        let sessions_dir = self.sessions_dir();
        if let Some(session) = self.session.as_mut() {
            let session_name = session.name().to_string();
            let session_path = match session_name.split_once("/") {
                Some((dir, name)) => sessions_dir.join(dir).join(format!("{name}.yaml")),
                None => sessions_dir.join(format!("{session_name}.yaml")),
            };
            session.persist(&session_path)?;
            Ok(Some(session_name))
        } else {
            Ok(None)
        }
    }

    pub fn light_theme(&self) -> bool {
        matches!(self.theme.as_deref(), Some("light"))
    }

    pub fn render_options(&self) -> Result<RenderOptions> {
        let theme = if self.highlight {
            let theme_mode = if self.light_theme() { "light" } else { "dark" };
            let theme_filename = format!("{theme_mode}.tmTheme");
            let theme_path = Self::local_path(&theme_filename);
            if theme_path.exists() {
                let theme = ThemeSet::get_theme(&theme_path)
                    .with_context(|| format!("Invalid theme at '{}'", theme_path.display()))?;
                Some(theme)
            } else {
                let theme = if self.light_theme() {
                    decode_bin(LIGHT_THEME).context("Invalid builtin light theme")?
                } else {
                    decode_bin(DARK_THEME).context("Invalid builtin dark theme")?
                };
                Some(theme)
            }
        } else {
            None
        };
        let wrap = if *IS_STDOUT_TERMINAL {
            self.wrap.clone()
        } else {
            None
        };
        let truecolor = matches!(
            env::var("COLORTERM").as_ref().map(|v| v.as_str()),
            Ok("truecolor")
        );
        Ok(RenderOptions::new(theme, wrap, self.wrap_code, truecolor))
    }
    pub fn print_markdown(&self, text: &str) -> Result<()> {
        if *IS_STDOUT_TERMINAL {
            let render_options = self.render_options()?;
            let mut markdown_render = MarkdownRender::init(render_options)?;
            println!("{}", markdown_render.render(text));
        } else {
            println!("{text}");
        }
        Ok(())
    }

    pub fn before_chat_completion(&mut self, input: &Input) -> Result<()> {
        self.last_message = Some(LastMessage::new(input.clone(), String::new()));
        Ok(())
    }

    pub fn after_chat_completion(&mut self, input: &Input, output: &str) -> Result<()> {
        self.last_message = Some(LastMessage::new(input.clone(), output.to_string()));
        if !self.dry_run {
            self.save_session_message(input, output)?;
        }
        Ok(())
    }

    fn discontinuous_last_message(&mut self) {
        if let Some(last_message) = self.last_message.as_mut() {
            last_message.continuous = false;
        }
    }

    fn save_session_message(&mut self, input: &Input, output: &str) -> Result<()> {
        let mut input = input.clone();
        input.clear_patch();
        let sessions_dir = self.sessions_dir();
        if let Some(session) = input.session_mut(&mut self.session) {
            session.add_message(&input, output)?;
            let session_path = match session.name().split_once("/") {
                Some((dir, name)) => sessions_dir.join(dir).join(format!("{name}.yaml")),
                None => sessions_dir.join(format!("{}.yaml", session.name())),
            };
            session.persist(&session_path)?;
        }
        Ok(())
    }

    pub fn append_session_note(&mut self, note: String) -> Result<()> {
        if self.dry_run {
            return Ok(());
        }
        let sessions_dir = self.sessions_dir();
        if let Some(session) = self.session.as_mut() {
            session.add_assistant_note(note);
            let session_path = match session.name().split_once("/") {
                Some((dir, name)) => sessions_dir.join(dir).join(format!("{name}.yaml")),
                None => sessions_dir.join(format!("{}.yaml", session.name())),
            };
            session.persist(&session_path)?;
        }
        Ok(())
    }

    fn load_from_file(config_path: &Path) -> Result<Self> {
        let err = || format!("Failed to load config at '{}'", config_path.display());
        let content = read_to_string(config_path).with_context(err)?;
        let config: Self = serde_yaml::from_str(&content)
            .map_err(|err| {
                let err_msg = err.to_string();
                let err_msg = if err_msg.starts_with(&format!("{CLIENTS_FIELD}: ")) {
                    // location is incorrect, get rid of it
                    err_msg
                        .split_once(" at line")
                        .map(|(v, _)| {
                            format!("{v} (Sorry for being unable to provide an exact location)")
                        })
                        .unwrap_or_else(|| "clients: invalid value".into())
                } else {
                    err_msg
                };
                anyhow!("{err_msg}")
            })
            .with_context(err)?;

        Ok(config)
    }

    fn load_dynamic(model_id: &str) -> Result<Self> {
        let provider = match model_id.split_once(':') {
            Some((v, _)) => v,
            _ => model_id,
        };
        let is_openai_compatible = OPENAI_COMPATIBLE_PROVIDERS
            .into_iter()
            .any(|(name, _)| provider == name);
        let client = if is_openai_compatible {
            json!({ "type": "openai-compatible", "name": provider })
        } else {
            json!({ "type": provider })
        };
        let config = json!({
            "model": model_id.to_string(),
            "save": false,
            "clients": vec![client],
        });
        let config =
            serde_json::from_value(config).with_context(|| "Failed to load config from env")?;
        Ok(config)
    }

    fn load_envs(&mut self) {
        if let Ok(v) = env::var(get_env_name("model")) {
            self.model_id = v;
        }
        if let Some(v) = read_env_value::<f64>(&get_env_name("temperature")) {
            self.temperature = v;
        }
        if let Some(v) = read_env_value::<f64>(&get_env_name("top_p")) {
            self.top_p = v;
        }

        if let Some(Some(v)) = read_env_bool(&get_env_name("dry_run")) {
            self.dry_run = v;
        }
        if let Some(Some(v)) = read_env_bool(&get_env_name("ai_summary")) {
            self.ai_summary = v;
        }
        if let Some(Some(v)) = read_env_bool(&get_env_name("stream")) {
            self.stream = v;
        }
        if let Some(v) = read_env_value::<String>(&get_env_name("wrap")) {
            self.wrap = v;
        }
        if let Some(Some(v)) = read_env_bool(&get_env_name("wrap_code")) {
            self.wrap_code = v;
        }

        if let Ok(v) = env::var(get_env_name("document_loaders")) {
            if let Ok(v) = serde_json::from_str(&v) {
                self.document_loaders = v;
            }
        }

        if let Some(Some(v)) = read_env_bool(&get_env_name("highlight")) {
            self.highlight = v;
        }
        if *NO_COLOR {
            self.highlight = false;
        }
        if self.highlight && self.theme.is_none() {
            if let Some(v) = read_env_value::<String>(&get_env_name("theme")) {
                self.theme = v;
            } else if *IS_STDOUT_TERMINAL {
                if let Ok(color_scheme) = color_scheme(QueryOptions::default()) {
                    let theme = match color_scheme {
                        ColorScheme::Dark => "dark",
                        ColorScheme::Light => "light",
                    };
                    self.theme = Some(theme.into());
                }
            }
        }

        if let Some(v) = read_env_value::<String>(&get_env_name("user_agent")) {
            self.user_agent = v;
        }
        if let Some(Some(v)) = read_env_bool(&get_env_name("save_shell_history")) {
            self.save_shell_history = v;
        }
    }

    fn setup_model(&mut self) -> Result<()> {
        let mut model_id = self.model_id.clone();
        if model_id.is_empty() {
            let models = list_models(self, ModelType::Chat);
            if models.is_empty() {
                bail!("No available model");
            }
            model_id = models[0].id()
        };
        self.set_model(&model_id)?;
        self.model_id = model_id;
        Ok(())
    }

    fn setup_document_loaders(&mut self) {
        [("pdf", "pdftotext $1 -"), ("docx", "pandoc --to plain $1")]
            .into_iter()
            .for_each(|(k, v)| {
                let (k, v) = (k.to_string(), v.to_string());
                self.document_loaders.entry(k).or_insert(v);
            });
    }

    fn setup_user_agent(&mut self) {
        if let Some("auto") = self.user_agent.as_deref() {
            self.user_agent = Some(format!(
                "{}/{}",
                env!("CARGO_CRATE_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }
    }
}

pub fn load_env_file() -> Result<()> {
    let env_file_path = Config::env_file();
    let contents = match read_to_string(&env_file_path) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    debug!("Use env file '{}'", env_file_path.display());
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env::set_var(key.trim(), value.trim());
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct LastMessage {
    pub input: Input,
    pub output: String,
    pub continuous: bool,
}

impl LastMessage {
    pub fn new(input: Input, output: String) -> Self {
        Self {
            input,
            output,
            continuous: true,
        }
    }
}

fn missing_config_guidance(config_path: &Path) -> String {
    format!(
        r#"AICmd config not found: {}

First-time setup:
1. Create a .env file with your model settings.
2. Run: aicmd init --from-env
3. Check: aicmd doctor
4. Try: aicmd 当前目录有多少文件"#,
        config_path.display()
    )
}

pub(crate) fn ensure_parent_exists(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Failed to write to '{}', No parent path", path.display()))?;
    if !parent.exists() {
        create_dir_all(parent).with_context(|| {
            format!(
                "Failed to write to '{}', Cannot create parent directory",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn home_aicmd_dir() -> PathBuf {
    dirs::home_dir()
        .expect("No user's home directory")
        .join(format!(".{}", env!("CARGO_CRATE_NAME")))
}

fn read_env_value<T>(key: &str) -> Option<Option<T>>
where
    T: std::str::FromStr,
{
    let value = env::var(key).ok()?;
    let value = parse_value(&value).ok()?;
    Some(value)
}

fn parse_value<T>(value: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
{
    let value = if value == "null" {
        None
    } else {
        let value = match value.parse() {
            Ok(value) => value,
            Err(_) => bail!("Invalid value '{}'", value),
        };
        Some(value)
    };
    Ok(value)
}

fn read_env_bool(key: &str) -> Option<Option<bool>> {
    let value = env::var(key).ok()?;
    Some(parse_bool(&value))
}
