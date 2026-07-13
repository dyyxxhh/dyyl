//! OpenAI Responses API provider.
//!
//! 端点：{base_url}/responses（base_url 空 = https://api.openai.com/v1）。

use super::client::{HttpRequest, request_with_retry};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenaiResponseProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenaiResponseProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self { api_key, model, base_url }
    }

    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let body = serde_json::json!({
            "model": self.model,
            "instructions": system,
            "input": user,
        });
        HttpRequest {
            url: format!("{base}/responses"),
            method: "POST".to_string(),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {}", self.api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应：output[*].content[*].text（找第一个有 text 的）。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None)
        })?;
        let output = v.get("output").and_then(|o| o.as_array());
        if let Some(arr) = output {
            for item in arr {
                if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                    for c in content {
                        if let Some(text) = c.get("text").and_then(|t| t.as_str()) {
                            return Ok(text.to_string());
                        }
                    }
                }
            }
        }
        Err(AiError::new(
            AiErrorKind::Parse,
            "missing output[*].content[*].text".to_string(),
            None,
        ))
    }
}

impl AiProvider for OpenaiResponseProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
