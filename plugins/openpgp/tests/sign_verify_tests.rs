//! Integration tests for the `sign.*` and `verify.*` commands.
//!
//! These tests build a `PluginState` backed by a `tempfile::tempdir()`
//! so keyring writes are isolated.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use openpgp::codec::DyylValue;
use openpgp::commands;
use openpgp::state::PluginState;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

/// Build a `PluginState` whose `credentials_dir` points at a fresh
/// tempdir, then generate a key whose passphrase is `test-pass`. Returns
/// the state, the tempdir (must be kept alive), and the new key's
/// fingerprint.
fn make_state_with_key() -> (PluginState, tempfile::TempDir, String) {
    let dir = tempdir().expect("tempdir");
    let mut state = PluginState {
        default_passphrase: Some("test-pass".to_string()),
        default_key: None,
        credentials_dir: PathBuf::from(dir.path()),
        ..Default::default()
    };
    let fp = match commands::dispatch(
        &mut state,
        "key.generate",
        &[
            DyylValue::Str("test <test@example.com>".to_string()),
            DyylValue::Str("test-pass".to_string()),
        ],
    )
    .expect("generate key")
    {
        DyylValue::Str(fp) => fp,
        _ => panic!("expected fingerprint string"),
    };
    (state, dir, fp)
}

/// Wrap a `&str` as a `DyylValue::Str` arg.
fn str_arg(s: &str) -> DyylValue {
    DyylValue::Str(s.to_string())
}

/// Extract a string field from a `DyylValue::Dict` by key.
fn dict_get<'a>(v: &'a DyylValue, key: &str) -> Option<&'a str> {
    match v {
        DyylValue::Dict(pairs) => pairs
            .iter()
            .find(|(k, _)| matches!(k, DyylValue::Str(s) if s == key))
            .and_then(|(_, v)| v.as_str()),
        _ => None,
    }
}

#[test]
fn sign_verify_inline_roundtrip() {
    let (mut state, _dir, fp) = make_state_with_key();

    let signed = commands::dispatch(
        &mut state,
        "sign",
        &[str_arg("hello"), str_arg(&fp)],
    )
    .expect("sign should succeed");
    let armored = signed.as_str().expect("armored signature string");
    assert!(
        armored.contains("BEGIN PGP MESSAGE"),
        "inline signed message should contain PGP MESSAGE header, got: {armored}"
    );

    let result = commands::dispatch(
        &mut state,
        "verify",
        &[str_arg(armored)],
    )
    .expect("verify should return Ok(dict)");
    assert!(
        matches!(result, DyylValue::Dict(_)),
        "verify should return a Dict, got: {result:?}"
    );
    assert_eq!(
        dict_get(&result, "valid"),
        Some("1"),
        "inline verify should be valid"
    );
    assert_eq!(
        dict_get(&result, "signer_fp"),
        Some(fp.as_str()),
        "signer_fp should match the key fingerprint"
    );
    let uid = dict_get(&result, "signer_uid").unwrap_or("");
    assert!(
        uid.contains("test"),
        "signer_uid should contain 'test', got: {uid}"
    );
}

#[test]
fn sign_detached_verify_roundtrip() {
    let (mut state, _dir, fp) = make_state_with_key();

    let signed = commands::dispatch(
        &mut state,
        "sign",
        &[str_arg("hello"), str_arg(&fp), str_arg("1")],
    )
    .expect("sign detached should succeed");
    let armored = signed.as_str().expect("armored detached signature");
    assert!(
        armored.contains("BEGIN PGP SIGNATURE") || armored.contains("BEGIN PGP MESSAGE"),
        "detached signature should contain PGP SIGNATURE or PGP MESSAGE header, got: {armored}"
    );

    let result = commands::dispatch(
        &mut state,
        "verify",
        &[str_arg(armored), str_arg("hello")],
    )
    .expect("verify should return Ok(dict)");
    assert!(
        matches!(result, DyylValue::Dict(_)),
        "verify should return a Dict, got: {result:?}"
    );
    assert_eq!(
        dict_get(&result, "valid"),
        Some("1"),
        "detached verify should be valid"
    );
    assert_eq!(
        dict_get(&result, "signer_fp"),
        Some(fp.as_str()),
        "signer_fp should match the key fingerprint"
    );
}

#[test]
fn sign_wrong_passphrase_returns_error() {
    let (mut state, _dir, fp) = make_state_with_key();

    let result = commands::dispatch(
        &mut state,
        "sign",
        &[str_arg("hello"), str_arg(&fp), str_arg("_"), str_arg("wrong-pass")],
    );
    let err = result.expect_err("sign with wrong passphrase should error");
    let code = err.code();
    assert!(
        code == "passphrase_wrong" || code == "runtime",
        "expected passphrase_wrong or runtime, got: {code} ({})",
        err.message()
    );
}

#[test]
fn verify_tampered_returns_invalid() {
    let (mut state, _dir, fp) = make_state_with_key();

    let signed = commands::dispatch(
        &mut state,
        "sign",
        &[str_arg("hello"), str_arg(&fp), str_arg("1")],
    )
    .expect("sign detached should succeed");
    let armored = signed.as_str().expect("armored detached signature").to_string();

    let result = commands::dispatch(
        &mut state,
        "verify",
        &[str_arg(&armored), str_arg("TAMPERED TEXT")],
    )
    .expect("verify should return Ok(dict) even when invalid");
    assert!(
        matches!(result, DyylValue::Dict(_)),
        "verify should return a Dict, got: {result:?}"
    );
    assert_eq!(
        dict_get(&result, "valid"),
        Some("0"),
        "tampered verify should be invalid"
    );
}

#[test]
fn sign_file_verify_file_roundtrip() {
    let (mut state, dir, fp) = make_state_with_key();

    let in_path = dir.path().join("in.txt");
    let signed_path = dir.path().join("signed.asc");
    fs::write(&in_path, "hello").expect("write input");

    let sign_result = commands::dispatch(
        &mut state,
        "sign.file",
        &[
            str_arg(in_path.to_str().unwrap()),
            str_arg(signed_path.to_str().unwrap()),
            str_arg(&fp),
        ],
    )
    .expect("sign.file should succeed");
    assert_eq!(
        sign_result.as_str(),
        Some("1"),
        "sign.file should return \"1\""
    );

    let signed_content = fs::read_to_string(&signed_path).expect("read signed file");
    assert!(
        signed_content.contains("BEGIN PGP MESSAGE"),
        "signed file should contain PGP MESSAGE header, got: {signed_content}"
    );

    let verify_result = commands::dispatch(
        &mut state,
        "verify.file",
        &[str_arg(signed_path.to_str().unwrap())],
    )
    .expect("verify.file should return Ok(dict)");
    assert!(
        matches!(verify_result, DyylValue::Dict(_)),
        "verify.file should return a Dict, got: {verify_result:?}"
    );
    assert_eq!(
        dict_get(&verify_result, "valid"),
        Some("1"),
        "verify.file should be valid"
    );
    assert_eq!(
        dict_get(&verify_result, "signer_fp"),
        Some(fp.as_str()),
        "verify.file signer_fp should match the key fingerprint"
    );
}
