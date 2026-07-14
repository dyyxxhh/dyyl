//! Keyring CRUD — reads/writes keys/<fp>.{pub,sec}.asc + index.json.

use std::fs;
use std::path::PathBuf;

use crate::state::{KeyringEntry, KeyringIndex};

pub struct Keyring {
    pub base_dir: PathBuf,
}

impl Keyring {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn keys_dir(&self) -> PathBuf {
        self.base_dir.join("keys")
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join("index.json")
    }

    fn key_path(&self, fp: &str, secret: bool) -> PathBuf {
        let suffix = if secret { "sec.asc" } else { "pub.asc" };
        self.keys_dir().join(format!("{fp}.{suffix}"))
    }

    /// Load the keyring index from disk. Returns an empty index if the
    /// file does not exist (fresh keyring) rather than an error.
    pub fn load_index(&self) -> Result<KeyringIndex, String> {
        if !self.index_path().exists() {
            return Ok(KeyringIndex::default());
        }
        let content =
            fs::read_to_string(self.index_path()).map_err(|e| format!("read index.json: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("parse index.json: {e}"))
    }

    /// Save the keyring index to disk (pretty-printed JSON).
    pub fn save_index(&self, index: &KeyringIndex) -> Result<(), String> {
        fs::create_dir_all(&self.base_dir).map_err(|e| format!("create base_dir: {e}"))?;
        let json = serde_json::to_string_pretty(index)
            .map_err(|e| format!("serialize index.json: {e}"))?;
        fs::write(self.index_path(), json).map_err(|e| format!("write index.json: {e}"))
    }

    /// Insert or update an entry, merging by fingerprint. If an entry
    /// with the same `fp` already exists it is replaced; otherwise the
    /// new entry is appended.
    pub fn upsert_entry(&self, entry: KeyringEntry) -> Result<(), String> {
        let mut index = self.load_index()?;
        if let Some(existing) = index.keys.iter_mut().find(|e| e.fp == entry.fp) {
            *existing = entry;
        } else {
            index.keys.push(entry);
        }
        self.save_index(&index)
    }

    /// Remove an entry and its key files from disk. Idempotent: removing
    /// a fingerprint that is not in the index (and whose files do not
    /// exist) succeeds without error.
    pub fn remove_entry(&self, fp: &str) -> Result<(), String> {
        let mut index = self.load_index()?;
        index.keys.retain(|e| e.fp != fp);
        self.save_index(&index)?;

        let pub_path = self.key_path(fp, false);
        if pub_path.exists() {
            fs::remove_file(&pub_path).map_err(|e| format!("remove pub key: {e}"))?;
        }
        let sec_path = self.key_path(fp, true);
        if sec_path.exists() {
            fs::remove_file(&sec_path).map_err(|e| format!("remove sec key: {e}"))?;
        }
        Ok(())
    }

    /// Write a key file under `keys/<fp>.{pub,sec}.asc`. Secret key
    /// files are chmod'd to 0600 on Unix.
    pub fn write_key_file(&self, fp: &str, secret: bool, content: &str) -> Result<(), String> {
        fs::create_dir_all(self.keys_dir()).map_err(|e| format!("create keys dir: {e}"))?;
        let path = self.key_path(fp, secret);
        fs::write(&path, content).map_err(|e| format!("write key file {}: {e}", path.display()))?;
        if secret {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                    .map_err(|e| format!("set key file permissions: {e}"))?;
            }
        }
        Ok(())
    }

    /// Read a key file. Returns `Err` if the file does not exist.
    pub fn read_key_file(&self, fp: &str, secret: bool) -> Result<String, String> {
        let path = self.key_path(fp, secret);
        if !path.exists() {
            return Err(format!("key file not found: {}", path.display()));
        }
        fs::read_to_string(&path).map_err(|e| format!("read key file {}: {e}", path.display()))
    }

    /// Find an entry by fingerprint. Returns `None` if not found.
    pub fn find_entry(&self, fp: &str) -> Result<Option<KeyringEntry>, String> {
        let index = self.load_index()?;
        Ok(index.keys.into_iter().find(|e| e.fp == fp))
    }
}
