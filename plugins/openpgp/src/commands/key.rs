//! `key.*` commands — keyring management (5 commands).

use std::str::FromStr;
use std::time::SystemTime;

use chrono::{DateTime, Utc};

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::keyring::Keyring;
use crate::state::{KeyringEntry, PluginState};

/// `key.generate` (arity 2): generate a new Ed25519/Curve25519 keypair,
/// store in keyring, return fingerprint.
///
/// Passphrase priority:
/// 1. If `args[1]` is provided and not `"_"` and not empty → use it.
/// 2. If `args[1]` is `"_"` or empty → use `state.default_passphrase`.
/// 3. If both are None/empty → no passphrase (unencrypted key).
pub fn generate(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    use sequoia_openpgp::cert::prelude::*;
    use sequoia_openpgp::crypto::Password;

    let user_id = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("key.generate expects (user_id, passphrase)"))?;
    let passphrase_arg = args.get(1).and_then(DyylValue::as_str).unwrap_or("_");

    let passphrase: Option<String> = if passphrase_arg == "_" || passphrase_arg.is_empty() {
        state.default_passphrase.clone()
    } else {
        Some(passphrase_arg.to_string())
    };

    let (cert, _revocation) = CertBuilder::new()
        .add_userid(user_id)
        .set_cipher_suite(CipherSuite::Cv25519)
        .add_signing_subkey()
        .add_storage_encryption_subkey()
        .set_password(passphrase.as_ref().map(|p| Password::from(p.as_bytes())))
        .generate()
        .map_err(|e| PluginError::runtime(format!("key generation failed: {e}")))?;

    let fp = cert.fingerprint().to_hex().to_uppercase();

    let pub_armored = serialize_cert_to_armor(&cert, false)
        .map_err(|e| PluginError::runtime(format!("serialize public key: {e}")))?;
    let sec_armored = serialize_cert_to_armor(&cert, true)
        .map_err(|e| PluginError::runtime(format!("serialize secret key: {e}")))?;

    let keyring = Keyring::new(state.credentials_dir.clone());
    keyring
        .write_key_file(&fp, false, &pub_armored)
        .map_err(|e| PluginError::runtime(format!("write pub key: {e}")))?;
    keyring
        .write_key_file(&fp, true, &sec_armored)
        .map_err(|e| PluginError::runtime(format!("write sec key: {e}")))?;

    let uid = cert
        .userids()
        .next()
        .map(|u| u.userid().to_string())
        .unwrap_or_default();
    let created = format_created(cert.primary_key().key().creation_time());

    keyring
        .upsert_entry(KeyringEntry {
            fp: fp.clone(),
            uid,
            has_secret: true,
            created,
        })
        .map_err(|e| PluginError::runtime(format!("update index: {e}")))?;

    Ok(DyylValue::Str(fp))
}

/// `key.import` (arity 1): import armored public or private key into keyring.
pub fn import(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let armored = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("key.import expects (armored)"))?;

    let cert = sequoia_openpgp::Cert::from_str(armored)
        .map_err(|e| PluginError::parse_failed(format!("parse armored key: {e}")))?;

    let fp = cert.fingerprint().to_hex().to_uppercase();
    let is_secret = cert.is_tsk();

    let keyring = Keyring::new(state.credentials_dir.clone());
    let pub_armored = serialize_cert_to_armor(&cert, false)
        .map_err(|e| PluginError::runtime(format!("re-serialize public: {e}")))?;
    keyring
        .write_key_file(&fp, false, &pub_armored)
        .map_err(|e| PluginError::runtime(format!("write pub key: {e}")))?;

    if is_secret {
        let sec_armored = serialize_cert_to_armor(&cert, true)
            .map_err(|e| PluginError::runtime(format!("re-serialize secret: {e}")))?;
        keyring
            .write_key_file(&fp, true, &sec_armored)
            .map_err(|e| PluginError::runtime(format!("write sec key: {e}")))?;
    }

    let uid = cert
        .userids()
        .next()
        .map(|u| u.userid().to_string())
        .unwrap_or_default();
    let created = format_created(cert.primary_key().key().creation_time());
    keyring
        .upsert_entry(KeyringEntry {
            fp: fp.clone(),
            uid,
            has_secret: is_secret,
            created,
        })
        .map_err(|e| PluginError::runtime(format!("update index: {e}")))?;

    Ok(DyylValue::Str(fp))
}

/// `key.export` (arity 2): export key from keyring as armored text
/// (secret flag exports private key).
pub fn export(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let fp = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("key.export expects (fingerprint, secret)"))?;
    let secret = args
        .get(1)
        .and_then(DyylValue::as_str)
        .map(|s| s == "1")
        .unwrap_or(false);

    let keyring = Keyring::new(state.credentials_dir.clone());
    let content = keyring
        .read_key_file(fp, secret)
        .map_err(|e| PluginError::key_not_found(format!("export key {fp}: {e}")))?;

    Ok(DyylValue::Str(content))
}

/// `key.list` (arity 0): list all keys in the keyring.
pub fn list(state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let keyring = Keyring::new(state.credentials_dir.clone());
    let index = keyring
        .load_index()
        .map_err(|e| PluginError::runtime(format!("load index: {e}")))?;

    let list: Vec<DyylValue> = index
        .keys
        .iter()
        .map(|entry| {
            let secret_str = if entry.has_secret { "1" } else { "0" };
            DyylValue::Dict(vec![
                (
                    DyylValue::Str("fp".to_string()),
                    DyylValue::Str(entry.fp.clone()),
                ),
                (
                    DyylValue::Str("uid".to_string()),
                    DyylValue::Str(entry.uid.clone()),
                ),
                (
                    DyylValue::Str("secret".to_string()),
                    DyylValue::Str(secret_str.to_string()),
                ),
                (
                    DyylValue::Str("created".to_string()),
                    DyylValue::Str(entry.created.clone()),
                ),
            ])
        })
        .collect();

    Ok(DyylValue::List(list))
}

/// `key.delete` (arity 1): delete a key from the keyring by fingerprint.
pub fn delete(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let fp = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("key.delete expects (fingerprint)"))?;

    let keyring = Keyring::new(state.credentials_dir.clone());

    keyring
        .find_entry(fp)
        .map_err(|e| PluginError::runtime(format!("find key: {e}")))?
        .ok_or_else(|| PluginError::key_not_found(format!("key {fp} not in keyring")))?;

    keyring
        .remove_entry(fp)
        .map_err(|e| PluginError::runtime(format!("delete key: {e}")))?;

    Ok(DyylValue::Str("1".to_string()))
}

// ── Helpers ──────────────────────────────────────────────────────

/// Serialize a `Cert` to an armored string. When `secret` is true, the
/// transferable secret key (TSK) form is emitted.
fn serialize_cert_to_armor(cert: &sequoia_openpgp::Cert, secret: bool) -> anyhow::Result<String> {
    use sequoia_openpgp::serialize::SerializeInto;

    let bytes = if secret {
        cert.as_tsk().armored().to_vec()?
    } else {
        cert.armored().to_vec()?
    };
    Ok(String::from_utf8(bytes)?)
}

/// Format a `SystemTime` as an ISO-8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_created(time: SystemTime) -> String {
    DateTime::<Utc>::from(time)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}
