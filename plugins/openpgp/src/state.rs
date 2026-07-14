use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// A keyring index entry — one per stored key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyringEntry {
    pub fp: String,
    pub uid: String,
    pub has_secret: bool,
    pub created: String,
}

/// The keyring index, persisted as `index.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct KeyringIndex {
    pub keys: Vec<KeyringEntry>,
}

/// Plugin handle state — allocated by `init`, freed by `shutdown`.
#[derive(Debug)]
pub struct PluginState {
    pub default_passphrase: Option<String>,
    pub default_key: Option<String>,
    pub credentials_dir: PathBuf,
    pub key_cache: Mutex<HashMap<String, String>>,
    pub index: Mutex<Option<KeyringIndex>>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            default_passphrase: None,
            default_key: None,
            credentials_dir: PathBuf::new(),
            key_cache: Mutex::new(HashMap::new()),
            index: Mutex::new(None),
        }
    }
}

impl PluginState {
    pub fn clear_cache(&self) {
        self.key_cache
            .lock()
            .expect("key_cache mutex poisoned")
            .clear();
        *self.index.lock().expect("index mutex poisoned") = None;
    }
}
