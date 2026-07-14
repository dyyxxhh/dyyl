//! `sign.*` commands (2 commands).

use std::io::Write;
use std::str::FromStr;

use sequoia_openpgp::armor;
use sequoia_openpgp::crypto::Password;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::{Armorer, LiteralWriter, Message, Signer};
use sequoia_openpgp::Cert;

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::keyring::Keyring;
use crate::state::PluginState;

const POLICY: &StandardPolicy = &StandardPolicy::new();

/// Load a secret key from the keyring, decrypt it with the given
/// passphrase, and return a `KeyPair` ready for signing.
fn load_signer(
    state: &PluginState,
    fp: &str,
    passphrase: &str,
) -> Result<sequoia_openpgp::crypto::KeyPair, PluginError> {
    let keyring = Keyring::new(state.credentials_dir.clone());
    let armored = keyring
        .read_key_file(fp, true)
        .map_err(|e| PluginError::key_not_found(format!("secret key {fp}: {e}")))?;
    let cert = Cert::from_str(&armored)
        .map_err(|e| PluginError::parse_failed(format!("parse secret key {fp}: {e}")))?;

    let ka = cert
        .keys()
        .secret()
        .with_policy(POLICY, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .ok_or_else(|| PluginError::runtime("no signing-capable secret key found"))?;

    let password = Password::from(passphrase.as_bytes());
    let keypair = ka
        .key()
        .clone()
        .parts_into_secret()
        .map_err(|e| PluginError::runtime(format!("parts_into_secret: {e}")))?
        .decrypt_secret(&password)
        .map_err(|e| PluginError::passphrase_wrong(format!("decrypt secret: {e}")))?
        .into_keypair()
        .map_err(|e| PluginError::runtime(format!("into_keypair: {e}")))?;
    Ok(keypair)
}

/// Resolve a passphrase for signing.
///
/// Priority:
/// 1. If `arg` is provided and not `"_"` and not empty → use it.
/// 2. If `arg` is `"_"` or not provided → use `state.default_passphrase`.
/// 3. If both are missing → error.
fn resolve_passphrase(state: &PluginState, arg: Option<&str>) -> Result<String, PluginError> {
    match arg {
        Some(p) if p != "_" && !p.is_empty() => Ok(p.to_string()),
        _ => state
            .default_passphrase
            .clone()
            .ok_or_else(|| PluginError::passphrase_wrong("no passphrase available")),
    }
}

/// `sign` (arity ≥2): sign text with a key (inline or detached).
///
/// `(text, key_fp, detach?, passphrase?)` — when `detach == "1"`,
/// produces an armored detached signature; otherwise an inline signed
/// message containing the data.
pub fn sign(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign expects (text, key_fp, detach?, passphrase?)"))?;
    let fp = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign expects (text, key_fp)"))?;
    let detach = args
        .get(2)
        .and_then(DyylValue::as_str)
        .map(|s| s == "1")
        .unwrap_or(false);
    let passphrase = resolve_passphrase(state, args.get(3).and_then(DyylValue::as_str))?;

    let signer = load_signer(state, fp, &passphrase)?;

    let mut sink = Vec::new();
    if detach {
        // Detached signature: armor (Kind::Signature) → Signer (detached).
        // The data is written directly to the Signer (no LiteralWriter).
        let message = Message::new(&mut sink);
        let message = Armorer::new(message)
            .kind(armor::Kind::Signature)
            .build()
            .map_err(|e| PluginError::runtime(format!("build armorer: {e}")))?;
        let mut writer = Signer::new(message, signer)
            .map_err(|e| PluginError::runtime(format!("create signer: {e}")))?
            .detached()
            .build()
            .map_err(|e| PluginError::runtime(format!("build detached signer: {e}")))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| PluginError::runtime(format!("write to signer: {e}")))?;
        writer
            .finalize()
            .map_err(|e| PluginError::runtime(format!("finalize signer: {e}")))?;
    } else {
        // Inline signed message: armor (Kind::Message) → Signer → LiteralWriter.
        let message = Message::new(&mut sink);
        let message = Armorer::new(message)
            .build()
            .map_err(|e| PluginError::runtime(format!("build armorer: {e}")))?;
        let message = Signer::new(message, signer)
            .map_err(|e| PluginError::runtime(format!("create signer: {e}")))?
            .build()
            .map_err(|e| PluginError::runtime(format!("build signer: {e}")))?;
        let mut writer = LiteralWriter::new(message)
            .build()
            .map_err(|e| PluginError::runtime(format!("build literal writer: {e}")))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| PluginError::runtime(format!("write plaintext: {e}")))?;
        writer
            .finalize()
            .map_err(|e| PluginError::runtime(format!("finalize signed message: {e}")))?;
    }

    let armored = String::from_utf8(sink)
        .map_err(|e| PluginError::runtime(format!("output not utf8: {e}")))?;
    Ok(DyylValue::Str(armored))
}

/// `sign.file` (arity ≥3): sign a file to an output file (inline or
/// detached). Returns `"1"` on success.
pub fn sign_file(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let in_path = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign.file expects (in_path, out_path, key_fp, detach?, passphrase?)"))?;
    let out_path = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign.file expects (in_path, out_path, key_fp, ...)"))?;
    let fp = args
        .get(2)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign.file expects (in_path, out_path, key_fp)"))?;

    let text = std::fs::read_to_string(in_path)
        .map_err(|e| PluginError::runtime(format!("read input file: {e}")))?;

    // Build sign args: text, fp, detach?, passphrase?
    let mut sign_args = vec![DyylValue::Str(text), DyylValue::Str(fp.to_string())];
    if let Some(detach) = args.get(3) {
        sign_args.push(detach.clone());
    }
    if let Some(pass) = args.get(4) {
        sign_args.push(pass.clone());
    }

    let result = sign(state, &sign_args)?;
    let armored = match result {
        DyylValue::Str(s) => s,
        _ => return Err(PluginError::runtime("sign returned non-string")),
    };
    std::fs::write(out_path, &armored)
        .map_err(|e| PluginError::runtime(format!("write output file: {e}")))?;
    Ok(DyylValue::Str("1".to_string()))
}
