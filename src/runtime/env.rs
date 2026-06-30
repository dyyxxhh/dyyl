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
}
