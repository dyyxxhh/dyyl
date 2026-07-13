//! AI Provider 模块 — 统一 trait + 三种 Provider 实现 + HTTP 客户端。

pub mod client;
pub mod prompt;
pub mod provider_anthropic;
pub mod provider_openai_chat;
pub mod provider_openai_response;

use crate::credentials::AiCredentials;

pub use crate::credentials::AiProviderKind;

/// AI 错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiError {
    pub kind: AiErrorKind,
    pub message: String,
    pub status: Option<u16>,
}

impl AiError {
    #[must_use]
    pub fn new(kind: AiErrorKind, message: String, status: Option<u16>) -> Self {
        Self {
            kind,
            message,
            status,
        }
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AiError {}

/// AI 错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorKind {
    Network,
    Auth,
    RateLimit,
    ServerError,
    Parse,
    Other,
}

/// AI Provider trait — 发送一次请求，返回响应文本。
pub trait AiProvider: Send + Sync {
    /// 发送一次 AI 请求。
    ///
    /// `system` 可能为空（用 provider 默认行为），`user` 是用户 prompt。
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError>;
}

/// 根据凭证构造 Provider 实例。
#[must_use]
pub fn build_provider(creds: &AiCredentials) -> Box<dyn AiProvider> {
    match creds.provider {
        crate::credentials::AiProviderKind::OpenaiChat => {
            Box::new(provider_openai_chat::OpenaiChatProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
        crate::credentials::AiProviderKind::OpenaiResponse => {
            Box::new(provider_openai_response::OpenaiResponseProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
        crate::credentials::AiProviderKind::Anthropic => {
            Box::new(provider_anthropic::AnthropicProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
    }
}
