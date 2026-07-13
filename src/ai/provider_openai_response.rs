//! OpenAI Responses API — Task 6 实现。
//!
//! Task 3 占位 stub：仅提供编译所需的最小类型骨架，Task 6 替换为完整实现。

use super::{AiError, AiErrorKind, AiProvider};

/// OpenAI Responses API provider。
pub struct OpenaiResponseProvider;

impl OpenaiResponseProvider {
    #[must_use]
    pub fn new(_api_key: String, _model: String, _base_url: String) -> Self {
        Self
    }
}

impl AiProvider for OpenaiResponseProvider {
    fn ask(&self, _system: &str, _user: &str) -> Result<String, AiError> {
        Err(AiError::new(
            AiErrorKind::Other,
            "openai-response provider not yet implemented (Task 6)".to_string(),
            None,
        ))
    }
}
