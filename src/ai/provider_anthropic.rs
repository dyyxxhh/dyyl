//! Anthropic Messages API — Task 7 实现。
//!
//! Task 3 占位 stub：仅提供编译所需的最小类型骨架，Task 7 替换为完整实现。

use super::{AiError, AiErrorKind, AiProvider};

/// Anthropic Messages API provider。
pub struct AnthropicProvider;

impl AnthropicProvider {
    #[must_use]
    pub fn new(_api_key: String, _model: String, _base_url: String) -> Self {
        Self
    }
}

impl AiProvider for AnthropicProvider {
    fn ask(&self, _system: &str, _user: &str) -> Result<String, AiError> {
        Err(AiError::new(
            AiErrorKind::Other,
            "anthropic provider not yet implemented (Task 7)".to_string(),
            None,
        ))
    }
}
