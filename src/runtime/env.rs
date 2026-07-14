//! dyyl global environment.
//!
//! Provides a single global scope for variable bindings.  `create.num` and
//! `create.str` initialise variables; `set` rebinds them.  Dict and list
//! containers are reference types mutated in place via their own commands.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::i18n::Lang;
use crate::runtime::host_provider::{GameChooseScope, HostProvider};
use crate::runtime::plugin::PluginManager;
use crate::runtime::value::Value;

/// Global dyyl environment — single scope for all variable bindings.
///
/// Variable names are stored **without** the `$` prefix.  The `$` is syntax
/// that the expression evaluator strips before calling `Env::get`.
#[derive(Debug, Clone)]
pub struct Env {
    bindings: HashMap<String, Value>,
    lang: Cell<Lang>,
    host_provider: Option<Arc<dyn HostProvider>>,
    game_scope: GameChooseScope,
    mcm_id_counter: Cell<u64>,
    plugin_manager: Arc<PluginManager>,
    script_args: Vec<String>,
    script_name: String,
}

impl Env {
    /// Create a new empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            lang: Cell::new(Lang::En),
            host_provider: None,
            game_scope: GameChooseScope::default(),
            mcm_id_counter: Cell::new(1),
            plugin_manager: Arc::new(PluginManager::new()),
            script_args: Vec::new(),
            script_name: String::new(),
        }
    }

    #[must_use]
    pub const fn lang(&self) -> Lang {
        self.lang.get()
    }

    pub fn set_lang(&self, lang: Lang) {
        self.lang.set(lang);
    }

    /// Look up a variable by name (without `$` prefix).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.get(name)
    }

    /// Set (rebind) a variable.  Returns the previous value if any.
    pub fn set(&mut self, name: &str, value: Value) -> Option<Value> {
        self.bindings.insert(name.to_string(), value)
    }

    /// Check whether a variable exists.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Create a new numeric variable with initial value `0`.
    ///
    /// Returns `false` if the variable already exists.
    pub fn create_num(&mut self, name: &str) -> bool {
        if self.bindings.contains_key(name) {
            false
        } else {
            self.bindings.insert(name.to_string(), Value::Num(0));
            true
        }
    }

    /// Create a new string variable with initial value `""`.
    ///
    /// Returns `false` if the variable already exists.
    pub fn create_str(&mut self, name: &str) -> bool {
        if self.bindings.contains_key(name) {
            false
        } else {
            self.bindings
                .insert(name.to_string(), Value::Str(String::new()));
            true
        }
    }

    pub fn set_host_provider(&mut self, provider: Arc<dyn HostProvider>) {
        self.host_provider = Some(provider);
    }

    #[must_use]
    pub fn host_provider(&self) -> Option<&Arc<dyn HostProvider>> {
        self.host_provider.as_ref()
    }

    #[must_use]
    pub const fn game_scope(&self) -> &GameChooseScope {
        &self.game_scope
    }

    pub const fn game_scope_mut(&mut self) -> &mut GameChooseScope {
        &mut self.game_scope
    }

    #[must_use]
    pub fn mcm_next_id(&self) -> String {
        let id = self.mcm_id_counter.get();
        self.mcm_id_counter.set(id + 1);
        id.to_string()
    }

    /// Access the plugin manager (shared via `Arc`).
    #[must_use]
    pub fn plugin_manager(&self) -> &PluginManager {
        &self.plugin_manager
    }

    /// Set the command-line args passed to the script (after the filename).
    pub fn set_script_args(&mut self, args: Vec<String>) {
        self.script_args = args;
    }

    /// Get the command-line args passed to the script.
    #[must_use]
    pub fn script_args(&self) -> &[String] {
        &self.script_args
    }

    /// Set the script filename (as passed on the command line, raw).
    pub fn set_script_name(&mut self, name: String) {
        self.script_name = name;
    }

    /// Get the raw script filename string.
    #[must_use]
    pub fn script_name(&self) -> &str {
        &self.script_name
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_create_and_get_num() {
        let mut env = Env::new();
        assert!(env.create_num("x"));
        assert_eq!(env.get("x"), Some(&Value::Num(0)));
        // Creating again does nothing.
        assert!(!env.create_num("x"));
    }

    #[test]
    fn env_create_and_get_str() {
        let mut env = Env::new();
        assert!(env.create_str("s"));
        assert_eq!(env.get("s"), Some(&Value::Str(String::new())));
    }

    #[test]
    fn env_set_rebind() {
        let mut env = Env::new();
        assert!(env.create_num("x"));
        assert_eq!(env.set("x", Value::Num(42)), Some(Value::Num(0)));
        assert_eq!(env.get("x"), Some(&Value::Num(42)));
    }

    #[test]
    fn env_set_new_var() {
        let mut env = Env::new();
        assert!(env.set("y", Value::Str("hello".into())).is_none());
        assert_eq!(env.get("y"), Some(&Value::Str("hello".to_string())));
    }

    #[test]
    fn env_has() {
        let mut env = Env::new();
        assert!(!env.has("z"));
        env.create_num("z");
        assert!(env.has("z"));
    }

    #[test]
    fn env_new_is_empty() {
        let env = Env::new();
        assert!(env.get("anything").is_none());
    }

    #[test]
    fn env_script_args_default_empty() {
        let env = Env::new();
        assert!(env.script_args().is_empty());
        assert!(env.script_name().is_empty());
    }

    #[test]
    fn env_set_script_args() {
        let mut env = Env::new();
        env.set_script_args(vec!["--help".to_string(), "foo".to_string()]);
        assert_eq!(env.script_args(), &["--help", "foo"]);
    }

    #[test]
    fn env_set_script_name() {
        let mut env = Env::new();
        env.set_script_name("/path/to/a.dyyl".to_string());
        assert_eq!(env.script_name(), "/path/to/a.dyyl");
    }
}
