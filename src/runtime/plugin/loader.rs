//! dlopen + symbol resolution + dispatch.

use std::sync::OnceLock;

/// Loaded plugin — holds the dlopen'd library.
///
/// (Real implementation in Task 7.)
pub struct PluginLoader {
    _name: String,
}

impl PluginLoader {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { _name: String::new() }
    }
}
