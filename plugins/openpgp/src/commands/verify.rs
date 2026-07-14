//! `verify.*` commands (2 commands).

use std::io::Read;
use std::str::FromStr;

use sequoia_openpgp::parse::stream::{
    DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationHelper, VerifierBuilder,
};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::{Cert, KeyHandle};

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::keyring::Keyring;
use crate::state::PluginState;

const POLICY: &StandardPolicy = &StandardPolicy::new();

/// Helper that supplies public certs from the keyring to the verifier
/// and records whether at least one signature checked out (and who
/// signed it).
struct VerifyHelper {
    certs: Vec<Cert>,
    valid: bool,
    signer_uid: String,
    signer_fp: String,
}

impl VerifyHelper {
    /// Build a fresh helper with no recorded signer.
    fn new(certs: Vec<Cert>) -> Self {
        Self {
            certs,
            valid: false,
            signer_uid: String::new(),
            signer_fp: String::new(),
        }
    }

    /// Build a fresh helper with `valid=false` and no certs — used when
    /// verifier setup fails before the original helper can be recovered.
    fn empty() -> Self {
        Self::new(Vec::new())
    }
}

impl VerificationHelper for VerifyHelper {
    fn get_certs(&mut self, ids: &[KeyHandle]) -> sequoia_openpgp::Result<Vec<Cert>> {
        // Return all certs whose key handle matches a requested id.
        let matching: Vec<Cert> = self
            .certs
            .iter()
            .filter(|c| {
                c.keys().any(|k| {
                    let kh = k.key().key_handle();
                    ids.iter().any(|id| id == &kh)
                })
            })
            .cloned()
            .collect();
        if matching.is_empty() {
            // Fallback: provide all certs and let the verifier try them.
            Ok(self.certs.clone())
        } else {
            Ok(matching)
        }
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        for layer in structure {
            if let MessageLayer::SignatureGroup { results } = layer {
                for good in results.into_iter().flatten() {
                    let cert = good.ka.cert();
                    self.valid = true;
                    self.signer_fp = cert.fingerprint().to_hex().to_uppercase();
                    self.signer_uid = cert
                        .userids()
                        .next()
                        .map(|u| u.userid().to_string())
                        .unwrap_or_default();
                }
            }
        }
        // Always Ok — the caller inspects `self.valid` to decide.
        Ok(())
    }
}

/// Load all public certs from the keyring. Returns an empty vec if the
/// keyring can't be read (the verifier will then report `valid=0`).
fn load_pub_certs(state: &PluginState) -> Result<Vec<Cert>, PluginError> {
    let keyring = Keyring::new(state.credentials_dir.clone());
    let index = keyring
        .load_index()
        .map_err(|e| PluginError::runtime(format!("load index: {e}")))?;
    let mut certs = Vec::new();
    for entry in &index.keys {
        if let Ok(armored) = keyring.read_key_file(&entry.fp, false) {
            if let Ok(cert) = Cert::from_str(&armored) {
                certs.push(cert);
            }
        }
    }
    Ok(certs)
}

/// Build the result dict: `{valid, signer_uid, signer_fp}`.
fn make_result(valid: bool, uid: &str, fp: &str) -> DyylValue {
    DyylValue::Dict(vec![
        (
            DyylValue::Str("valid".to_string()),
            DyylValue::Str(if valid { "1" } else { "0" }.to_string()),
        ),
        (
            DyylValue::Str("signer_uid".to_string()),
            DyylValue::Str(uid.to_string()),
        ),
        (
            DyylValue::Str("signer_fp".to_string()),
            DyylValue::Str(fp.to_string()),
        ),
    ])
}

/// Verify a detached signature against `data`. Returns the helper with
/// updated `valid`/`signer_*` fields. On setup failure, returns a fresh
/// empty helper (the original is consumed by the verifier builder).
fn verify_detached(sig: &str, data: &str, helper: VerifyHelper) -> VerifyHelper {
    let reader = std::io::Cursor::new(sig.as_bytes());
    let builder = match DetachedVerifierBuilder::from_reader(reader) {
        Ok(b) => b,
        Err(_) => return VerifyHelper::empty(),
    };
    let mut verifier = match builder.with_policy(POLICY, None, helper) {
        Ok(v) => v,
        Err(_) => return VerifyHelper::empty(),
    };
    // Feed the original data; helper.check() runs during this call.
    let _ = verifier.verify_bytes(data.as_bytes());
    verifier.into_helper()
}

/// Verify an inline signed message. Returns the helper with updated
/// `valid`/`signer_*` fields.
fn verify_inline(signed_msg: &str, helper: VerifyHelper) -> VerifyHelper {
    let reader = std::io::Cursor::new(signed_msg.as_bytes());
    let builder = match VerifierBuilder::from_reader(reader) {
        Ok(b) => b,
        Err(_) => return VerifyHelper::empty(),
    };
    let mut verifier = match builder.with_policy(POLICY, None, helper) {
        Ok(v) => v,
        Err(_) => return VerifyHelper::empty(),
    };
    // Reading the plaintext triggers signature verification; the
    // helper's `check` runs at end-of-stream.
    let mut plaintext = Vec::new();
    let _ = verifier.read_to_end(&mut plaintext);
    verifier.into_helper()
}

/// `verify` (arity 1–2): verify a signed message (inline or detached).
///
/// - 1 arg: inline verify (sig+data combined).
/// - 2 args: detached verify (sig, original text).
///
/// Returns `Ok(Dict({valid, signer_uid, signer_fp}))` — never `Err` for
/// a verification failure; the `valid` field carries that signal.
pub fn verify(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text_or_sig = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("verify expects (text_or_sig, signed_text?)"))?;
    let signed_text = args.get(1).and_then(DyylValue::as_str);

    let certs = load_pub_certs(state).unwrap_or_default();
    let helper = VerifyHelper::new(certs);

    // Run the verifier. Any error here just means `valid=0`; we still
    // return a dict so the dyyl script can inspect the `valid` field.
    let helper = if let Some(data) = signed_text {
        verify_detached(text_or_sig, data, helper)
    } else {
        verify_inline(text_or_sig, helper)
    };

    Ok(make_result(
        helper.valid,
        &helper.signer_uid,
        &helper.signer_fp,
    ))
}

/// `verify.file` (arity 1–2): verify a signed file (inline or
/// detached). Same semantics as `verify` but reads from files.
pub fn verify_file(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let sig_or_data_path = args.first().and_then(DyylValue::as_str).ok_or_else(|| {
        PluginError::arity_mismatch("verify.file expects (sig_or_data_path, data_path?)")
    })?;
    let data_path = args.get(1).and_then(DyylValue::as_str);

    let content = std::fs::read_to_string(sig_or_data_path)
        .map_err(|e| PluginError::runtime(format!("read file: {e}")))?;

    let verify_args = if let Some(dp) = data_path {
        let data = std::fs::read_to_string(dp)
            .map_err(|e| PluginError::runtime(format!("read data file: {e}")))?;
        vec![DyylValue::Str(content), DyylValue::Str(data)]
    } else {
        vec![DyylValue::Str(content)]
    };

    verify(state, &verify_args)
}
