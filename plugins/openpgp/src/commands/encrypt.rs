//! `encrypt.*` and `sym.encrypt` commands (3 commands).

use std::io::Write;
use std::str::FromStr;

use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::{Armorer, Encryptor, LiteralWriter, Message};
use sequoia_openpgp::Cert;

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::keyring::Keyring;
use crate::state::PluginState;

const POLICY: &StandardPolicy = &StandardPolicy::new();

/// Resolve a recipient argument to a `Cert`. If `recipient` is a
/// 40-char hex fingerprint, look up the public key in the keyring;
/// otherwise parse it as an inline armored public key.
fn resolve_recipient(state: &PluginState, recipient: &str) -> Result<Cert, PluginError> {
    let is_fp = recipient.len() == 40 && recipient.chars().all(|c| c.is_ascii_hexdigit());
    if is_fp {
        let keyring = Keyring::new(state.credentials_dir.clone());
        let armored = keyring
            .read_key_file(recipient, false)
            .map_err(|e| PluginError::key_not_found(format!("recipient key {recipient}: {e}")))?;
        Cert::from_str(&armored)
            .map_err(|e| PluginError::parse_failed(format!("parse key {recipient}: {e}")))
    } else {
        Cert::from_str(recipient)
            .map_err(|e| PluginError::parse_failed(format!("parse armored recipient: {e}")))
    }
}

/// Encrypt `text` for the given recipient certs and return armored
/// ciphertext.
fn encrypt_to_armored(
    state: &PluginState,
    text: &str,
    recipient_strs: &[&str],
) -> Result<String, PluginError> {
    let certs: Vec<Cert> = recipient_strs
        .iter()
        .map(|r| resolve_recipient(state, r))
        .collect::<Result<_, _>>()?;

    let mut sink = Vec::new();
    let message = Message::new(&mut sink);
    // Wrap in an armorer so the output is ASCII-armored.
    let message = Armorer::new(message)
        .build()
        .map_err(|e| PluginError::runtime(format!("build armorer: {e}")))?;
    let recipients = certs.iter().flat_map(|c| {
        c.keys()
            .with_policy(POLICY, None)
            .supported()
            .alive()
            .revoked(false)
            .for_storage_encryption()
    });
    let message = Encryptor::for_recipients(message, recipients)
        .build()
        .map_err(|e| PluginError::runtime(format!("build encryptor: {e}")))?;
    let mut writer = LiteralWriter::new(message)
        .build()
        .map_err(|e| PluginError::runtime(format!("build literal writer: {e}")))?;
    writer
        .write_all(text.as_bytes())
        .map_err(|e| PluginError::runtime(format!("write plaintext: {e}")))?;
    writer
        .finalize()
        .map_err(|e| PluginError::runtime(format!("finalize encryption: {e}")))?;

    String::from_utf8(sink).map_err(|e| PluginError::runtime(format!("ciphertext not utf8: {e}")))
}

/// `encrypt` (arity ≥2): encrypt text for one or more recipients
/// (fingerprint or armored pubkey), returning armored ciphertext.
pub fn encrypt(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("encrypt expects (text, recipient, ...)"))?;

    let recipient_strs: Vec<&str> = args.iter().skip(1).filter_map(DyylValue::as_str).collect();
    if recipient_strs.is_empty() {
        return Err(PluginError::arity_mismatch(
            "encrypt requires at least one recipient",
        ));
    }

    let armored = encrypt_to_armored(state, text, &recipient_strs)?;
    Ok(DyylValue::Str(armored))
}

/// `encrypt.file` (arity ≥3): encrypt a file to an output file for one
/// or more recipients. Returns `"1"` on success.
pub fn encrypt_file(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let in_path = args.first().and_then(DyylValue::as_str).ok_or_else(|| {
        PluginError::arity_mismatch("encrypt.file expects (in_path, out_path, recipient, ...)")
    })?;
    let out_path = args.get(1).and_then(DyylValue::as_str).ok_or_else(|| {
        PluginError::arity_mismatch("encrypt.file expects (in_path, out_path, recipient, ...)")
    })?;
    let recipient_strs: Vec<&str> = args.iter().skip(2).filter_map(DyylValue::as_str).collect();
    if recipient_strs.is_empty() {
        return Err(PluginError::arity_mismatch(
            "encrypt.file requires at least one recipient",
        ));
    }

    let text = std::fs::read_to_string(in_path)
        .map_err(|e| PluginError::runtime(format!("read input file: {e}")))?;

    let armored = encrypt_to_armored(state, &text, &recipient_strs)?;

    std::fs::write(out_path, &armored)
        .map_err(|e| PluginError::runtime(format!("write output file: {e}")))?;
    Ok(DyylValue::Str("1".to_string()))
}

/// `sym.encrypt` (arity ≥2): symmetrically encrypt text with a
/// passphrase, returning armored ciphertext.
pub fn sym_encrypt(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sym.encrypt expects (text, passphrase)"))?;
    let passphrase = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sym.encrypt expects (text, passphrase)"))?;

    let mut sink = Vec::new();
    let message = Message::new(&mut sink);
    let message = Armorer::new(message)
        .build()
        .map_err(|e| PluginError::runtime(format!("build armorer: {e}")))?;
    // `with_passwords` takes an iterator of items that are `Into<Password>`;
    // `&str` satisfies that via `Password: From<&str>`.
    let message = Encryptor::with_passwords(message, Some(passphrase))
        .build()
        .map_err(|e| PluginError::runtime(format!("build symmetric encryptor: {e}")))?;
    let mut writer = LiteralWriter::new(message)
        .build()
        .map_err(|e| PluginError::runtime(format!("build literal writer: {e}")))?;
    writer
        .write_all(text.as_bytes())
        .map_err(|e| PluginError::runtime(format!("write plaintext: {e}")))?;
    writer
        .finalize()
        .map_err(|e| PluginError::runtime(format!("finalize encryption: {e}")))?;

    let armored = String::from_utf8(sink)
        .map_err(|e| PluginError::runtime(format!("ciphertext not utf8: {e}")))?;
    Ok(DyylValue::Str(armored))
}
