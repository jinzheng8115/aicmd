use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::{
    env, fs,
    fs::read_to_string,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use crate::config::Config;

const SEARCHES_DIR_ENV: &str = "AICMD_SEARCHES_DIR";
const SEARCHES_DIR_NAME: &str = "searches";
const SEARCH_EXT: &str = "txt";
const LAST_SEARCH_NAME: &str = ".last";
const LAST_RAW_SEARCH_NAME: &str = ".last.raw";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSearchRecord {
    pub query: String,
    pub raw_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRunOptions {
    pub query: String,
    pub save_name: Option<Option<String>>,
}

#[derive(Debug)]
struct SavedSearch {
    name: String,
    path: PathBuf,
    status: String,
    modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
struct SearchFiles {
    summary_path: Option<PathBuf>,
    raw_path: Option<PathBuf>,
    modified: Option<SystemTime>,
}

pub fn run_search_store_command(args: &[String]) -> Result<i32> {
    match args.first().map(String::as_str) {
        Some("save") => save_last(args.get(1).map(String::as_str)),
        Some("list") | Some("ls") => list_searches(),
        Some("show") => show_search(args.get(1).map(String::as_str)),
        Some("rm") | Some("remove") | Some("delete") => {
            remove_search(args.get(1).map(String::as_str))
        }
        Some("open") => open_search(args.get(1).map(String::as_str)),
        Some("help") | Some("-h") | Some("--help") | None => {
            print_usage();
            Ok(0)
        }
        Some(arg) => bail!("Unknown search command: {arg}"),
    }
}

pub fn parse_search_run_args(args: &[String]) -> Result<SearchRunOptions> {
    let mut query_parts = vec![];
    let mut save_name = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--save" => {
                let next = args.get(index + 1);
                if let Some(value) = next.filter(|value| !value.starts_with('-')) {
                    save_name = Some(Some(value.to_string()));
                    index += 2;
                } else {
                    save_name = Some(None);
                    index += 1;
                }
            }
            value if value.starts_with("--save=") => {
                let value = value.trim_start_matches("--save=");
                if value.is_empty() {
                    save_name = Some(None);
                } else {
                    save_name = Some(Some(value.to_string()));
                }
                index += 1;
            }
            value => {
                query_parts.push(value.to_string());
                index += 1;
            }
        }
    }
    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        bail!("usage: aicmd search <query> [--save [name]]");
    }
    Ok(SearchRunOptions { query, save_name })
}

pub fn parse_summarize_target(args: &[String]) -> Result<String> {
    if args.len() > 1 {
        bail!("usage: aicmd search summarize [name|last]");
    }
    Ok(args.first().cloned().unwrap_or_else(|| "last".to_string()))
}

pub fn persist_raw_search_result(
    query: &str,
    raw_output: &str,
    save_name: Option<Option<String>>,
) -> Result<PathBuf> {
    let content = build_raw_search_record(query, raw_output);
    let last_path = search_file(LAST_RAW_SEARCH_NAME);
    write_search_file(&last_path, &content)?;
    if let Some(name) = save_name {
        let name = name.unwrap_or_else(|| generated_search_name(query));
        let name = normalize_search_name(&name)?;
        let path = raw_search_file(&name);
        write_search_file(&path, &content)?;
    }
    Ok(last_path)
}

pub fn persist_search_result(
    query: &str,
    summary: &str,
    save_name: Option<Option<String>>,
) -> Result<()> {
    let content = build_search_record(query, summary);
    let last_path = search_file(LAST_SEARCH_NAME);
    write_search_file(&last_path, &content)?;
    if let Some(name) = save_name {
        let name = name.unwrap_or_else(|| generated_search_name(query));
        let name = normalize_search_name(&name)?;
        let path = search_file(&name);
        write_search_file(&path, &content)?;
        println!("Saved search: {name}");
        println!("File: {}", path.display());
    }
    Ok(())
}

pub fn load_raw_search(name: &str) -> Result<RawSearchRecord> {
    let name = if name == "last" {
        LAST_RAW_SEARCH_NAME.to_string()
    } else {
        format!("{}.raw", normalize_search_name(name)?)
    };
    let path = search_file(&name);
    if !path.exists() {
        bail!("Raw search not found: {name} ({})", path.display());
    }
    parse_raw_search_record(&read_to_string(&path)?)
        .with_context(|| format!("Failed to parse raw search: {}", path.display()))
}

pub fn saved_search_path(name: &str) -> Result<PathBuf> {
    let name = if name == "last" {
        LAST_SEARCH_NAME.to_string()
    } else {
        normalize_search_name(name)?
    };
    Ok(search_file(&name))
}

pub fn raw_search_path(name: &str) -> Result<PathBuf> {
    let name = if name == "last" {
        LAST_RAW_SEARCH_NAME.to_string()
    } else {
        format!("{}.raw", normalize_search_name(name)?)
    };
    Ok(search_file(&name))
}

fn save_last(name: Option<&str>) -> Result<i32> {
    let last_path = search_file(LAST_SEARCH_NAME);
    if !last_path.exists() {
        bail!("No last search found. Run `aicmd search <query>` first.");
    }
    let content = read_to_string(&last_path)
        .with_context(|| format!("Failed to read last search: {}", last_path.display()))?;
    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| generated_search_name_from_content(&content));
    let name = normalize_search_name(&name)?;
    let path = search_file(&name);
    write_search_file(&path, &content)?;
    let last_raw_path = search_file(LAST_RAW_SEARCH_NAME);
    if last_raw_path.exists() {
        let raw_content = read_to_string(&last_raw_path).with_context(|| {
            format!(
                "Failed to read last raw search: {}",
                last_raw_path.display()
            )
        })?;
        write_search_file(&raw_search_file(&name), &raw_content)?;
    }
    println!("Saved search: {name}");
    println!("File: {}", path.display());
    Ok(0)
}

fn list_searches() -> Result<i32> {
    let dir = searches_dir();
    if !dir.exists() {
        println!("No saved searches found. / 没有找到已保存搜索。");
        println!("Searches dir: {}", dir.display());
        return Ok(0);
    }
    let mut searches = collect_searches(&dir)?;
    searches.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    if searches.is_empty() {
        println!("No saved searches found. / 没有找到已保存搜索。");
        println!("Searches dir: {}", dir.display());
        return Ok(0);
    }
    println!("Searches dir: {}", dir.display());
    println!("{:<32} {:<12} {:<19} File", "Name", "Status", "Updated");
    for item in searches {
        println!(
            "{:<32} {:<12} {:<19} {}",
            item.name,
            item.status,
            format_system_time(item.modified),
            item.path.display()
        );
    }
    Ok(0)
}

fn show_search(name: Option<&str>) -> Result<i32> {
    let Some(name) = name else {
        bail!("usage: aicmd search show <name|last>");
    };
    let name = if name == "last" {
        LAST_SEARCH_NAME
    } else {
        name
    };
    let path = search_file(name);
    if !path.exists() {
        bail!("Saved search not found: {name} ({})", path.display());
    }
    print!("{}", read_to_string(&path)?);
    Ok(0)
}

fn remove_search(name: Option<&str>) -> Result<i32> {
    let Some(name) = name else {
        bail!("usage: aicmd search rm <name>");
    };
    let name = normalize_search_name(name)?;
    let paths = [search_file(&name), raw_search_file(&name)];
    let mut removed = 0;
    for path in paths {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            println!("Removed: {}", path.display());
            removed += 1;
        }
    }
    if removed == 0 {
        bail!("Saved search not found: {name}");
    }
    Ok(0)
}

fn open_search(name: Option<&str>) -> Result<i32> {
    let Some(name) = name else {
        bail!("usage: aicmd search open <name|last>");
    };
    let summary_path = saved_search_path(name)?;
    let raw_path = raw_search_path(name)?;
    let path = if summary_path.exists() {
        summary_path
    } else if raw_path.exists() {
        raw_path
    } else {
        bail!("Saved search not found: {name}");
    };
    open_path(&path)?;
    println!("Opened: {}", path.display());
    Ok(0)
}

fn collect_searches(dir: &Path) -> Result<Vec<SavedSearch>> {
    let mut by_name: BTreeMap<String, SearchFiles> = BTreeMap::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() || path.extension().and_then(|v| v.to_str()) != Some(SEARCH_EXT) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|v| v.to_str()) else {
            continue;
        };
        if stem == LAST_SEARCH_NAME || stem == LAST_RAW_SEARCH_NAME {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
        let (name, is_raw) = if let Some(name) = stem.strip_suffix(".raw") {
            if name == LAST_SEARCH_NAME {
                continue;
            }
            (name.to_string(), true)
        } else {
            (stem.to_string(), false)
        };
        let item = by_name.entry(name).or_default();
        item.modified = max_system_time(item.modified, modified);
        if is_raw {
            item.raw_path = Some(path);
        } else {
            item.summary_path = Some(path);
        }
    }
    Ok(by_name
        .into_iter()
        .map(|(name, files)| {
            let status = match (&files.summary_path, &files.raw_path) {
                (Some(_), Some(_)) => "summary+raw",
                (Some(_), None) => "summary",
                (None, Some(_)) => "raw",
                (None, None) => "unknown",
            }
            .to_string();
            let path = files
                .summary_path
                .or(files.raw_path)
                .unwrap_or_else(|| search_file(&name));
            SavedSearch {
                name,
                path,
                status,
                modified: files.modified,
            }
        })
        .collect())
}

fn build_raw_search_record(query: &str, raw_output: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    format!(
        "AICmd raw search\nTime: {now}\nQuery: {query}\n\n---\n\n{}\n",
        raw_output.trim_end()
    )
}

fn build_search_record(query: &str, summary: &str) -> String {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    format!(
        "AICmd saved search\nTime: {now}\nQuery: {query}\n\n---\n\n{}\n",
        summary.trim_end()
    )
}

fn generated_search_name(query: &str) -> String {
    let time = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let slug = slugify(query);
    if slug.is_empty() {
        time.to_string()
    } else {
        format!("{time}-{slug}")
    }
}

fn generated_search_name_from_content(content: &str) -> String {
    let query = content
        .lines()
        .find_map(|line| line.strip_prefix("Query: "))
        .unwrap_or("search");
    generated_search_name(query)
}

fn parse_raw_search_record(content: &str) -> Result<RawSearchRecord> {
    let query = content
        .lines()
        .find_map(|line| line.strip_prefix("Query: "))
        .context("missing Query line")?
        .to_string();
    let (_, raw_output) = content
        .split_once("\n---\n\n")
        .context("missing raw search separator")?;
    Ok(RawSearchRecord {
        query,
        raw_output: raw_output.trim_end().to_string(),
    })
}

fn normalize_search_name(name: &str) -> Result<String> {
    let name = name.trim().trim_end_matches(".txt");
    if name.is_empty() || name == "last" || name == LAST_SEARCH_NAME {
        bail!("Invalid search name: {name}");
    }
    let normalized = slugify(name);
    if normalized.is_empty() {
        bail!("Invalid search name: {name}");
    }
    Ok(normalized)
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_alphanumeric() {
            Some(ch)
        } else {
            None
        };
        if let Some(ch) = mapped {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.chars().count() >= 48 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn write_search_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))
}

fn search_file(name: &str) -> PathBuf {
    searches_dir().join(format!("{name}.{SEARCH_EXT}"))
}

fn raw_search_file(name: &str) -> PathBuf {
    search_file(&format!("{name}.raw"))
}

fn searches_dir() -> PathBuf {
    env::var(SEARCHES_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| Config::config_dir().join(SEARCHES_DIR_NAME))
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "unknown".to_string();
    };
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn max_system_time(left: Option<SystemTime>, right: Option<SystemTime>) -> Option<SystemTime> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn open_path(path: &Path) -> Result<()> {
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.trim().is_empty() {
            Command::new(editor)
                .arg(path)
                .spawn()
                .with_context(|| format!("Failed to open {}", path.display()))?;
            return Ok(());
        }
    }
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", &path.display().to_string()]);
        command
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };
    command
        .spawn()
        .with_context(|| format!("Failed to open {}", path.display()))?;
    Ok(())
}

fn print_usage() {
    println!(
        r#"Usage: aicmd search <query> [--save [name]]
       aicmd search save [name]
       aicmd search summarize [name|last]
       aicmd search list
       aicmd search show <name|last>
       aicmd search open <name|last>
       aicmd search rm <name>

用法：aicmd search <查询> [--save [名称]]
      aicmd search save [名称]
      aicmd search summarize [名称|last]
      aicmd search list
      aicmd search show <名称|last>
      aicmd search open <名称|last>
      aicmd search rm <名称>

Commands / 命令:
  --save [name]   Save this search immediately / 搜索后立即保存
  save [name]     Save the last search result / 保存上一次搜索结果
  summarize       Summarize a saved raw search / 重新整理原始搜索结果
  list            List saved searches / 列出已保存搜索
  show <name>     Show saved search content / 查看已保存搜索
  open <name>     Open saved search in editor/app / 打开已保存搜索
  rm <name>       Remove saved search files / 删除已保存搜索
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_save_name() {
        let args = vec![
            "hello".to_string(),
            "--save".to_string(),
            "note".to_string(),
        ];
        let options = parse_search_run_args(&args).unwrap();
        assert_eq!(options.query, "hello");
        assert_eq!(options.save_name, Some(Some("note".to_string())));
    }

    #[test]
    fn parse_search_save_without_name() {
        let args = vec![
            "hello".to_string(),
            "world".to_string(),
            "--save".to_string(),
        ];
        let options = parse_search_run_args(&args).unwrap();
        assert_eq!(options.query, "hello world");
        assert_eq!(options.save_name, Some(None));
    }

    #[test]
    fn slug_keeps_chinese() {
        assert_eq!(
            slugify("Gemini CLI 官方安装方式!"),
            "gemini-cli-官方安装方式"
        );
    }

    #[test]
    fn parse_summarize_defaults_to_last() {
        let args = vec![];
        assert_eq!(parse_summarize_target(&args).unwrap(), "last");
    }

    #[test]
    fn parse_raw_search_roundtrip() {
        let content = build_raw_search_record("docker 如何安装", "raw result\nline 2");
        let record = parse_raw_search_record(&content).unwrap();
        assert_eq!(record.query, "docker 如何安装");
        assert_eq!(record.raw_output, "raw result\nline 2");
    }
}
