//! OpenAI Chat Completions — Task 5 实现。
//!
//! Task 3 占位 stub：仅提供编译所需的最小类型骨架，Task 5 替换为完整实现。

use super::{AiError, AiErrorKind, AiProvider};

/// OpenAI Chat Completions provider。
pub struct OpenaiChatProvider;

impl OpenaiChatProvider {
    #[must_use]
    pub fn new(_api_key: String, _model: String, _base_url: String) -> Self {
        Self
    }
}

impl AiProvider for OpenaiChatProvider {
    fn ask(&self, _system: &str, _user: &str) -> Result<String, AiError> {
        Err(AiError::new(
            AiErrorKind::Other,
            "openai-chat provider not yet implemented (Task 5)".to_string(),
            None,
        ))
    }
}
