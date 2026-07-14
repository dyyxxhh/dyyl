//! Integration tests for the `key.*` commands.
//!
//! Each test builds a `PluginState` backed by a `tempfile::tempdir()` so
//! keyring writes are isolated. `expect` is used in helpers (allowed by the
//! crate clippy config; only `unwrap_used` and `panic` are denied).

#![allow(clippy::unwrap_used)]

use openpgp::codec::DyylValue;
use openpgp::commands;
use openpgp::keyring::Keyring;
use openpgp::state::PluginState;
use std::path::PathBuf;
use tempfile::tempdir;

/// Build a `PluginState` whose `credentials_dir` points at a fresh tempdir.
/// The returned `TempDir` must be kept alive for the duration of the test.
fn make_state() -> (PluginState, tempfile::TempDir) {
    let dir = tempdir().expect("create tempdir");
    let state = PluginState {
        credentials_dir: PathBuf::from(dir.path()),
        default_passphrase: Some("test-pass".to_string()),
        ..Default::default()
    };
    (state, dir)
}

/// Wrap a `&str` as a `DyylValue::Str` arg.
fn str_arg(s: &str) -> DyylValue {
    DyylValue::Str(s.to_string())
}

/// Pull the fingerprint string out of a generate/import result.
fn extract_fp(v: &DyylValue) -> String {
    v.as_str()
        .map(String::from)
        .expect("expected string fingerprint")
}

/// Look up a value in a `DyylValue::Dict` by string key.
fn dict_get<'a>(dict: &'a DyylValue, key: &str) -> Option<&'a DyylValue> {
    match dict {
        DyylValue::Dict(pairs) => pairs
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v),
        _ => None,
    }
}

/// Path of a key file inside a keyring base dir.
fn key_file_path(state: &PluginState, fp: &str, secret: bool) -> PathBuf {
    let suffix = if secret { "sec.asc" } else { "pub.asc" };
    state.credentials_dir.join("keys").join(format!("{fp}.{suffix}"))
}

#[test]
fn key_generate_returns_fingerprint_and_writes_files() {
    let (mut state, _dir) = make_state();
    let result = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("test <test@example.com>"), str_arg("pass123")],
    );
    let fp = extract_fp(&result.expect("generate should succeed"));
    assert_eq!(fp.len(), 40, "fingerprint should be 40 hex chars");
    assert!(
        fp.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)),
        "fp should be uppercase hex, got {fp}"
    );

    assert!(key_file_path(&state, &fp, false).exists(), "pub key file should exist");
    assert!(key_file_path(&state, &fp, true).exists(), "sec key file should exist");

    let keyring = Keyring::new(state.credentials_dir.clone());
    assert!(
        keyring.base_dir.join("index.json").exists(),
        "index.json should exist"
    );
    let entry = keyring
        .find_entry(&fp)
        .expect("find_entry should not error")
        .expect("entry should exist in index");
    assert_eq!(entry.fp, fp);
    assert_eq!(entry.uid, "test <test@example.com>");
    assert!(entry.has_secret, "generated key should have a secret");
}

#[test]
fn key_generate_with_underscore_uses_default_passphrase() {
    let (mut state, _dir) = make_state();
    let result = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("underscore <u@example.com>"), str_arg("_")],
    );
    let fp = extract_fp(&result.expect("generate should succeed"));
    assert_eq!(fp.len(), 40);
    assert!(key_file_path(&state, &fp, false).exists(), "pub key file should exist");
    assert!(key_file_path(&state, &fp, true).exists(), "sec key file should exist");
}

#[test]
fn key_list_empty() {
    let (mut state, _dir) = make_state();
    let result = commands::dispatch(&mut state, "key.list", &[]);
    let v = result.expect("list should succeed on fresh keyring");
    let items = v.as_list().expect("expected a list");
    assert!(items.is_empty(), "fresh keyring should list empty");
}

#[test]
fn key_list_after_generate() {
    let (mut state, _dir) = make_state();
    let gen = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("list <list@example.com>"), str_arg("pass123")],
    )
    .expect("generate");
    let fp = extract_fp(&gen);

    let list_result = commands::dispatch(&mut state, "key.list", &[]).expect("list");
    let items = list_result.as_list().expect("expected a list");
    assert_eq!(items.len(), 1, "should have exactly one key");

    let entry = items.first().expect("one entry present");
    let fp_val = dict_get(entry, "fp").expect("dict should have fp");
    assert_eq!(fp_val.as_str(), Some(fp.as_str()));
    let uid_val = dict_get(entry, "uid").expect("dict should have uid");
    assert_eq!(uid_val.as_str(), Some("list <list@example.com>"));
    let secret_val = dict_get(entry, "secret").expect("dict should have secret");
    assert_eq!(secret_val.as_str(), Some("1"), "generated key has secret");
    let created_val = dict_get(entry, "created").expect("dict should have created");
    assert!(
        created_val.as_str().is_some_and(|s| !s.is_empty()),
        "created should be non-empty"
    );
}

#[test]
fn key_delete_removes_key() {
    let (mut state, _dir) = make_state();
    let gen = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("del <del@example.com>"), str_arg("pass123")],
    )
    .expect("generate");
    let fp = extract_fp(&gen);

    let del_result = commands::dispatch(&mut state, "key.delete", &[str_arg(&fp)])
        .expect("delete should succeed");
    assert_eq!(del_result.as_str(), Some("1"), "delete should return \"1\"");

    assert!(
        !key_file_path(&state, &fp, false).exists(),
        "pub file should be gone"
    );
    assert!(
        !key_file_path(&state, &fp, true).exists(),
        "sec file should be gone"
    );

    let list_result = commands::dispatch(&mut state, "key.list", &[]).expect("list");
    let items = list_result.as_list().expect("expected a list");
    assert!(items.is_empty(), "list should be empty after delete");
}

#[test]
fn key_delete_nonexistent_returns_error() {
    let (mut state, _dir) = make_state();
    let result = commands::dispatch(&mut state, "key.delete", &[str_arg("NONEXISTENT1234")]);
    let err = result.expect_err("delete nonexistent should error");
    assert_eq!(
        err.code(),
        "key_not_found",
        "expected key_not_found, got: {}",
        err.message()
    );
}

#[test]
fn key_export_public() {
    let (mut state, _dir) = make_state();
    let gen = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("exp <exp@example.com>"), str_arg("pass123")],
    )
    .expect("generate");
    let fp = extract_fp(&gen);

    let exported = commands::dispatch(&mut state, "key.export", &[str_arg(&fp), str_arg("0")])
        .expect("export should succeed");
    let armored = exported.as_str().expect("expected armored string");
    assert!(
        armored.contains("BEGIN PGP PUBLIC KEY BLOCK"),
        "public export should contain public key block header"
    );
}

#[test]
fn key_export_secret() {
    let (mut state, _dir) = make_state();
    let gen = commands::dispatch(
        &mut state,
        "key.generate",
        &[str_arg("sec <sec@example.com>"), str_arg("pass123")],
    )
    .expect("generate");
    let fp = extract_fp(&gen);

    let exported = commands::dispatch(&mut state, "key.export", &[str_arg(&fp), str_arg("1")])
        .expect("export should succeed");
    let armored = exported.as_str().expect("expected armored string");
    assert!(
        armored.contains("BEGIN PGP PRIVATE KEY BLOCK"),
        "secret export should contain private key block header"
    );
}
