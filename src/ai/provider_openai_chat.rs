//! OpenAI Chat Completions API provider.
//!
//! 端点：{base_url}/chat/completions（base_url 空 = https://api.openai.com/v1）。

use super::client::{HttpRequest, request_with_retry};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenaiChatProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenaiChatProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self { api_key, model, base_url }
    }

    /// 构造 HTTP 请求（不发送）— 供测试验证请求体。
    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        HttpRequest {
            url: format!("{base}/chat/completions"),
            method: "POST".to_string(),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {}", self.api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应体，提取 choices[0].message.content。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None)
        })?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str());
        match content {
            Some(s) => Ok(s.to_string()),
            None => Err(AiError::new(
                AiErrorKind::Parse,
                "missing choices[0].message.content".to_string(),
                None,
            )),
        }
    }
}

impl AiProvider for OpenaiChatProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
