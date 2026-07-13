//! Plugin system — dynamic-library loading and dispatch.
//!
//! Plugins are compiled dynamic libraries (.so/.dll/.dylib) loaded via dlopen.
//! The `PluginManager` orchestrates fetch → install → load → dispatch.
//! On first call to `<name>.<sub>`, the manager fetches the manifest from
//! l.dyyapp.com, downloads+verifies the library, dlopens it, and dispatches.

pub mod abi;
pub mod fetch;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod store;
pub mod value_codec;

use std::collections::HashMap;
use std::sync::Mutex;

use crate::i18n::Lang;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Loaded plugin instance — holds the dlopen'd library and handle.
pub struct LoadedPlugin {
    /// The name of the plugin (e.g. "migpt").
    pub name: String,
    /// The dlopen'd library + resolved symbols.
    pub loader: loader::PluginLoader,
}

/// Central plugin manager — holds already-loaded plugins.
pub struct PluginManager {
    loaded: Mutex<HashMap<String, LoadedPlugin>>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Dispatch a plugin command `<name>.<sub>` (sub may contain dots).
    ///
    /// If the plugin isn't loaded yet, this fetches+installs+loads it first.
    /// Returns the plugin's result Value or a RuntimeError.
    pub fn dispatch(
        &self,
        name: &str,
        sub: &str,
        args: &[Value],
        lang: Lang,
        line: usize,
    ) -> Result<Value, RuntimeError> {
        // Placeholder — implemented in Task 8.
        let _ = (name, sub, args, lang, line);
        Err(RuntimeError::new(
            line,
            format!("{name}.{sub}"),
            crate::i18n::plugin_command_failed(lang, name, sub, "not_implemented"),
        ))
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
