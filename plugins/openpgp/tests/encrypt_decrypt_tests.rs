//! Integration tests for the `encrypt.*`, `decrypt.*`, and `sym.*`
//! commands.
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
    // Generate a key and return its fingerprint.
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

#[test]
fn encrypt_decrypt_roundtrip_with_fp() {
    let (mut state, _dir, fp) = make_state_with_key();

    let ciphertext = commands::dispatch(
        &mut state,
        "encrypt",
        &[str_arg("hello"), str_arg(&fp)],
    )
    .expect("encrypt should succeed");
    let armored = ciphertext.as_str().expect("armored string");
    assert!(
        armored.starts_with("-----BEGIN PGP MESSAGE"),
        "ciphertext should start with PGP MESSAGE header, got: {armored}"
    );

    let plaintext = commands::dispatch(
        &mut state,
        "decrypt",
        &[str_arg(armored), str_arg("test-pass")],
    )
    .expect("decrypt should succeed");
    assert_eq!(plaintext.as_str(), Some("hello"));
}

#[test]
fn encrypt_with_inline_armored_pubkey() {
    let (mut state, _dir, fp) = make_state_with_key();

    // Export the public key.
    let exported = commands::dispatch(
        &mut state,
        "key.export",
        &[str_arg(&fp), str_arg("0")],
    )
    .expect("export should succeed");
    let armored_pubkey = exported.as_str().expect("armored pubkey").to_string();

    // Encrypt using the inline armored public key (not the fingerprint).
    let ciphertext = commands::dispatch(
        &mut state,
        "encrypt",
        &[str_arg("hello"), str_arg(&armored_pubkey)],
    )
    .expect("encrypt with inline pubkey should succeed");
    let armored = ciphertext.as_str().expect("armored ciphertext");
    assert!(
        armored.starts_with("-----BEGIN PGP MESSAGE"),
        "ciphertext should start with PGP MESSAGE header"
    );

    // Decrypt with the passphrase (the secret key is in the keyring).
    let plaintext = commands::dispatch(
        &mut state,
        "decrypt",
        &[str_arg(armored), str_arg("test-pass")],
    )
    .expect("decrypt should succeed");
    assert_eq!(plaintext.as_str(), Some("hello"));
}

#[test]
fn encrypt_nonexistent_fp_returns_error() {
    let (mut state, _dir, _fp) = make_state_with_key();

    let result = commands::dispatch(
        &mut state,
        "encrypt",
        &[str_arg("hello"), str_arg("DEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF")],
    );
    let err = result.expect_err("encrypt with nonexistent fp should error");
    assert_eq!(
        err.code(),
        "key_not_found",
        "expected key_not_found, got: {}",
        err.message()
    );
}

#[test]
fn sym_encrypt_decrypt_roundtrip() {
    let (mut state, _dir, _fp) = make_state_with_key();

    let ciphertext = commands::dispatch(
        &mut state,
        "sym.encrypt",
        &[str_arg("hello"), str_arg("mypass")],
    )
    .expect("sym.encrypt should succeed");
    let armored = ciphertext.as_str().expect("armored ciphertext");
    assert!(
        armored.starts_with("-----BEGIN PGP MESSAGE"),
        "symmetric ciphertext should start with PGP MESSAGE header"
    );

    let plaintext = commands::dispatch(
        &mut state,
        "sym.decrypt",
        &[str_arg(armored), str_arg("mypass")],
    )
    .expect("sym.decrypt should succeed");
    assert_eq!(plaintext.as_str(), Some("hello"));
}

#[test]
fn decrypt_wrong_passphrase_returns_error() {
    let (mut state, _dir, fp) = make_state_with_key();

    let ciphertext = commands::dispatch(
        &mut state,
        "encrypt",
        &[str_arg("hello"), str_arg(&fp)],
    )
    .expect("encrypt should succeed");
    let armored = ciphertext.as_str().expect("armored string").to_string();

    let result = commands::dispatch(
        &mut state,
        "decrypt",
        &[str_arg(&armored), str_arg("wrong-pass")],
    );
    let err = result.expect_err("decrypt with wrong passphrase should error");
    let code = err.code();
    assert!(
        code == "passphrase_wrong" || code == "runtime",
        "expected passphrase_wrong or runtime, got: {code}"
    );
}

#[test]
fn encrypt_file_roundtrip() {
    let (mut state, dir, fp) = make_state_with_key();

    let in_path = dir.path().join("in.txt");
    let enc_path = dir.path().join("out.asc");
    let dec_path = dir.path().join("result.txt");
    fs::write(&in_path, "hello").expect("write input");

    let enc_result = commands::dispatch(
        &mut state,
        "encrypt.file",
        &[str_arg(in_path.to_str().unwrap()), str_arg(enc_path.to_str().unwrap()), str_arg(&fp)],
    )
    .expect("encrypt.file should succeed");
    assert_eq!(enc_result.as_str(), Some("1"), "encrypt.file should return \"1\"");

    let enc_content = fs::read_to_string(&enc_path).expect("read encrypted file");
    assert!(
        enc_content.contains("BEGIN PGP MESSAGE"),
        "encrypted file should contain PGP MESSAGE header"
    );

    let dec_result = commands::dispatch(
        &mut state,
        "decrypt.file",
        &[
            str_arg(enc_path.to_str().unwrap()),
            str_arg(dec_path.to_str().unwrap()),
            str_arg("test-pass"),
        ],
    )
    .expect("decrypt.file should succeed");
    assert_eq!(dec_result.as_str(), Some("1"), "decrypt.file should return \"1\"");

    let dec_content = fs::read_to_string(&dec_path).expect("read decrypted file");
    assert_eq!(dec_content, "hello", "decrypted file should contain original text");
}
