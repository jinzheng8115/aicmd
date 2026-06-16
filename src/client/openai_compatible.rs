use super::openai::*;
use super::*;

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAICompatibleConfig {
    pub name: Option<String>,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelData>,
    pub patch: Option<RequestPatch>,
    pub extra: Option<ExtraConfig>,
}

impl OpenAICompatibleClient {
    config_get_fn!(api_base, get_api_base);
    config_get_fn!(api_key, get_api_key);

    pub const PROMPTS: [PromptAction<'static>; 0] = [];
}

impl_client_trait!(
    OpenAICompatibleClient,
    (
        prepare_chat_completions,
        openai_chat_completions,
        openai_chat_completions_streaming
    ),
);

fn prepare_chat_completions(
    self_: &OpenAICompatibleClient,
    data: ChatCompletionsData,
) -> Result<RequestData> {
    let api_key = self_.get_api_key().ok();
    let api_base = get_api_base_ext(self_)?;

    let url = format!("{api_base}/chat/completions");

    let body = openai_build_chat_completions_body(data, &self_.model);

    let mut request_data = RequestData::new(url, body);

    if let Some(api_key) = api_key {
        request_data.bearer_auth(api_key);
    }

    Ok(request_data)
}



fn get_api_base_ext(self_: &OpenAICompatibleClient) -> Result<String> {
    let api_base = match self_.get_api_base() {
        Ok(v) => v,
        Err(err) => {
            match OPENAI_COMPATIBLE_PROVIDERS
                .into_iter()
                .find_map(|(name, api_base)| {
                    if name == self_.model.client_name() {
                        Some(api_base.to_string())
                    } else {
                        None
                    }
                }) {
                Some(v) => v,
                None => return Err(err),
            }
        }
    };
    Ok(api_base.trim_end_matches('/').to_string())
}
