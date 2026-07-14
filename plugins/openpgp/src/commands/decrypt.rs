//! `decrypt.*` and `sym.decrypt` commands (3 commands).

use std::io::Read;
use std::str::FromStr;

use sequoia_openpgp::crypto::{Password, SessionKey};
use sequoia_openpgp::parse::stream::{
    DecryptionHelper, DecryptorBuilder, MessageStructure, VerificationHelper,
};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::types::SymmetricAlgorithm;
use sequoia_openpgp::{packet::PKESK, packet::SKESK};
use sequoia_openpgp::{Cert, KeyHandle};

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::keyring::Keyring;
use crate::state::PluginState;

const POLICY: &StandardPolicy = &StandardPolicy::new();

/// Resolve a passphrase for decryption.
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

/// Load all secret certs from the keyring.
fn load_secret_certs(state: &PluginState) -> Result<Vec<Cert>, PluginError> {
    let keyring = Keyring::new(state.credentials_dir.clone());
    let index = keyring
        .load_index()
        .map_err(|e| PluginError::runtime(format!("load index: {e}")))?;

    let mut certs = Vec::new();
    for entry in &index.keys {
        if !entry.has_secret {
            continue;
        }
        let armored = keyring
            .read_key_file(&entry.fp, true)
            .map_err(|e| PluginError::runtime(format!("read secret key {}: {e}", entry.fp)))?;
        match Cert::from_str(&armored) {
            Ok(cert) => certs.push(cert),
            Err(e) => {
                return Err(PluginError::parse_failed(format!(
                    "parse secret key {}: {e}",
                    entry.fp
                )))
            }
        }
    }
    Ok(certs)
}

/// Helper that decrypts session keys using secret certs (for PKESK)
/// and/or a passphrase (for SKESK).
struct DecryptHelper {
    certs: Vec<Cert>,
    passphrase: Password,
}

impl DecryptionHelper for DecryptHelper {
    fn decrypt(
        &mut self,
        pkesks: &[PKESK],
        skesks: &[SKESK],
        sym_algo: Option<SymmetricAlgorithm>,
        decrypt: &mut dyn FnMut(Option<SymmetricAlgorithm>, &SessionKey) -> bool,
    ) -> sequoia_openpgp::Result<Option<Cert>> {
        // First, try SKESKs with the passphrase (symmetric encryption).
        for skesk in skesks {
            if let Ok((algo, sk)) = skesk.decrypt(&self.passphrase) {
                if decrypt(algo, &sk) {
                    return Ok(None);
                }
            }
        }

        // Then, try PKESKs with each secret cert's keys.
        for pkesk in pkesks {
            for cert in &self.certs {
                for ka in cert
                    .keys()
                    .with_policy(POLICY, None)
                    .supported()
                    .for_storage_encryption()
                {
                    let key = ka.key();
                    // Convert to secret parts; skips keys without secret material.
                    let key_secret = match key.clone().parts_into_secret() {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    // Decrypt the secret key material with the passphrase.
                    let decrypted = match key_secret.decrypt_secret(&self.passphrase) {
                        Ok(k) => k,
                        Err(_) => continue,
                    };
                    let mut keypair = match decrypted.into_keypair() {
                        Ok(kp) => kp,
                        Err(_) => continue,
                    };
                    if let Some((algo, sk)) = pkesk.decrypt(&mut keypair, sym_algo) {
                        if decrypt(algo, &sk) {
                            return Ok(Some(cert.clone()));
                        }
                    }
                }
            }
        }

        Err(sequoia_openpgp::Error::InvalidArgument(
            "No key to decrypt message (wrong passphrase?)".into(),
        )
        .into())
    }
}

impl VerificationHelper for DecryptHelper {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> sequoia_openpgp::Result<Vec<Cert>> {
        // Provide all available certs for signature verification.
        Ok(self.certs.clone())
    }

    fn check(&mut self, _structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        // Don't enforce signature verification on decrypt.
        Ok(())
    }
}

/// Map a sequoia decryption error to the appropriate `PluginError`.
fn map_decrypt_error(e: anyhow::Error) -> PluginError {
    let msg = e.to_string();
    if msg.contains("passphrase")
        || msg.contains("password")
        || msg.contains("No key to decrypt")
        || msg.contains("Wrong key")
    {
        PluginError::passphrase_wrong(msg)
    } else {
        PluginError::runtime(msg)
    }
}

/// `decrypt` (arity 1+): decrypt an armored message, with an optional
/// passphrase override.
pub fn decrypt(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let armor = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("decrypt expects (armor, passphrase?)"))?;
    let passphrase = resolve_passphrase(state, args.get(1).and_then(DyylValue::as_str))?;

    let secret_certs = load_secret_certs(state)?;
    let helper = DecryptHelper {
        certs: secret_certs,
        passphrase: Password::from(passphrase.as_bytes()),
    };

    let reader = std::io::Cursor::new(armor.as_bytes());
    let mut decryptor = DecryptorBuilder::from_reader(reader)
        .map_err(|e| PluginError::parse_failed(format!("build decryptor: {e}")))?
        .with_policy(POLICY, None, helper)
        .map_err(map_decrypt_error)?;

    let mut plaintext = Vec::new();
    decryptor
        .read_to_end(&mut plaintext)
        .map_err(|e| PluginError::runtime(format!("read plaintext: {e}")))?;

    let text = String::from_utf8(plaintext)
        .map_err(|e| PluginError::runtime(format!("plaintext not utf8: {e}")))?;
    Ok(DyylValue::Str(text))
}

/// `decrypt.file` (arity 2+): decrypt an armored file to an output
/// file. Returns `"1"` on success.
pub fn decrypt_file(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let in_path = args.first().and_then(DyylValue::as_str).ok_or_else(|| {
        PluginError::arity_mismatch("decrypt.file expects (in_path, out_path, passphrase?)")
    })?;
    let out_path = args.get(1).and_then(DyylValue::as_str).ok_or_else(|| {
        PluginError::arity_mismatch("decrypt.file expects (in_path, out_path, passphrase?)")
    })?;

    let armor = std::fs::read_to_string(in_path)
        .map_err(|e| PluginError::runtime(format!("read input file: {e}")))?;

    let decrypt_args = vec![
        DyylValue::Str(armor),
        args.get(2)
            .cloned()
            .unwrap_or_else(|| DyylValue::Str("_".to_string())),
    ];
    let result = decrypt(state, &decrypt_args)?;

    let plaintext = match result {
        DyylValue::Str(s) => s,
        _ => return Err(PluginError::runtime("decrypt returned non-string")),
    };
    std::fs::write(out_path, &plaintext)
        .map_err(|e| PluginError::runtime(format!("write output file: {e}")))?;
    Ok(DyylValue::Str("1".to_string()))
}

/// `sym.decrypt` (arity 2+): symmetrically decrypt an armored message
/// with a passphrase.
pub fn sym_decrypt(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let armor = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sym.decrypt expects (armor, passphrase)"))?;
    let passphrase = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sym.decrypt expects (armor, passphrase)"))?;

    // For symmetric decryption, no secret certs are needed — the
    // passphrase alone decrypts the SKESKs.
    let helper = DecryptHelper {
        certs: Vec::new(),
        passphrase: Password::from(passphrase.as_bytes()),
    };

    let reader = std::io::Cursor::new(armor.as_bytes());
    let mut decryptor = DecryptorBuilder::from_reader(reader)
        .map_err(|e| PluginError::parse_failed(format!("build decryptor: {e}")))?
        .with_policy(POLICY, None, helper)
        .map_err(map_decrypt_error)?;

    let mut plaintext = Vec::new();
    decryptor
        .read_to_end(&mut plaintext)
        .map_err(|e| PluginError::runtime(format!("read plaintext: {e}")))?;

    let text = String::from_utf8(plaintext)
        .map_err(|e| PluginError::runtime(format!("plaintext not utf8: {e}")))?;
    Ok(DyylValue::Str(text))
}
