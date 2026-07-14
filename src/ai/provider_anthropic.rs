//! Anthropic Messages API provider.
//!
//! 端点：{base_url}/v1/messages（base_url 空 = https://api.anthropic.com）。
//! 请求头：x-api-key + anthropic-version: 2023-06-01。

use super::client::{request_with_retry, HttpRequest};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
        }
    }

    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "messages": [
                {"role": "user", "content": user},
            ],
        });
        if !system.is_empty() {
            if let Some(obj) = body.as_object_mut() {
                obj.entry("system".to_string())
                    .or_insert_with(|| Value::String(system.to_string()));
            }
        }
        HttpRequest {
            url: format!("{base}/v1/messages"),
            method: "POST".to_string(),
            headers: vec![
                ("x-api-key".to_string(), self.api_key.clone()),
                (
                    "anthropic-version".to_string(),
                    ANTHROPIC_VERSION.to_string(),
                ),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应：content[*].text（找第一个 type=text 的）。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body)
            .map_err(|e| AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None))?;
        let content = v.get("content").and_then(|c| c.as_array());
        if let Some(arr) = content {
            for item in arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        return Ok(text.to_string());
                    }
                }
            }
        }
        Err(AiError::new(
            AiErrorKind::Parse,
            "missing content[*].text".to_string(),
            None,
        ))
    }
}

impl AiProvider for AnthropicProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
