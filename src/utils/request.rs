use super::*;

use anyhow::{bail, Result};
use http::header::CONTENT_TYPE;
use serde_json::Value;
use std::sync::LazyLock;
use std::{collections::HashMap, time::Duration};
use tokio::io::AsyncWriteExt;

pub const URL_LOADER: &str = "url";
pub const MEDIA_URL_EXTENSION: &str = "media_url";
pub const DEFAULT_EXTENSION: &str = "txt";

static CLIENT: LazyLock<Result<reqwest::Client>> = LazyLock::new(|| {
    let builder = reqwest::ClientBuilder::new().timeout(Duration::from_secs(16));
    let client = builder.build()?;
    Ok(client)
});

pub async fn fetch_with_loaders(
    loaders: &HashMap<String, String>,
    path: &str,
    allow_media: bool,
) -> Result<(String, String)> {
    if let Some(loader_command) = loaders.get(URL_LOADER) {
        let contents = run_loader_command(path, URL_LOADER, loader_command)?;
        return Ok((contents, DEFAULT_EXTENSION.into()));
    }
    let client = match *CLIENT {
        Ok(ref client) => client,
        Err(ref err) => bail!("{err}"),
    };
    let mut res = client.get(path).send().await?;
    if !res.status().is_success() {
        bail!("Invalid status: {}", res.status());
    }
    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| match v.split_once(';') {
            Some((mime, _)) => mime.trim(),
            None => v,
        })
        .map(|v| v.to_string())
        .unwrap_or_else(|| {
            format!(
                "_/{}",
                get_patch_extension(path).unwrap_or_else(|| DEFAULT_EXTENSION.into())
            )
        });
    let mut is_media = false;
    let extension = match content_type.as_str() {
        "application/pdf" => "pdf".into(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx".into(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx".into(),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            "pptx".into()
        }
        "application/vnd.oasis.opendocument.text" => "odt".into(),
        "application/vnd.oasis.opendocument.spreadsheet" => "ods".into(),
        "application/vnd.oasis.opendocument.presentation" => "odp".into(),
        "application/rtf" => "rtf".into(),
        "text/javascript" => "js".into(),
        "text/html" => "html".into(),
        _ => content_type
            .rsplit_once('/')
            .map(|(first, last)| {
                if ["image", "video", "audio"].contains(&first) {
                    is_media = true;
                    MEDIA_URL_EXTENSION.into()
                } else {
                    last.to_lowercase()
                }
            })
            .unwrap_or_else(|| DEFAULT_EXTENSION.into()),
    };
    let result = if is_media {
        if !allow_media {
            bail!("Unexpected media type")
        }
        let image_bytes = res.bytes().await?;
        let image_base64 = base64_encode(&image_bytes);
        let contents = format!("data:{content_type};base64,{image_base64}");
        (contents, extension)
    } else {
        match loaders.get(&extension) {
            Some(loader_command) => {
                let save_path = temp_file("-download-", &format!(".{extension}"))
                    .display()
                    .to_string();
                let mut save_file = tokio::fs::File::create(&save_path).await?;
                let mut size = 0;
                while let Some(chunk) = res.chunk().await? {
                    size += chunk.len();
                    save_file.write_all(&chunk).await?;
                }
                let contents = if size == 0 {
                    println!("{}", warning_text(&format!("No content at '{path}'")));
                    String::new()
                } else {
                    run_loader_command(&save_path, &extension, loader_command)?
                };
                (contents, DEFAULT_EXTENSION.into())
            }
            None => {
                let contents = res.text().await?;
                if extension == "html" {
                    (html_to_md(&contents), "md".into())
                } else {
                    (contents, extension)
                }
            }
        }
    };
    Ok(result)
}

pub async fn fetch_models(api_base: &str, api_key: Option<&str>) -> Result<Vec<String>> {
    let client = match *CLIENT {
        Ok(ref client) => client,
        Err(ref err) => bail!("{err}"),
    };
    let mut builder = client.get(format!("{}/models", api_base.trim_end_matches('/')));
    if let Some(api_key) = api_key {
        builder = builder.bearer_auth(api_key);
    }
    let res_body: Value = builder.send().await?.json().await?;
    let mut result: Vec<String> = res_body
        .get("data")
        .and_then(|v| v.as_array())
        .map(|v| {
            v.iter()
                .filter_map(|v| v.get("id").and_then(|v| v.as_str().map(|v| v.to_string())))
                .collect()
        })
        .unwrap_or_default();
    if result.is_empty() {
        bail!("No valid models")
    }
    result.sort_unstable();
    Ok(result)
}
