//! Integration tests for the keyring CRUD module.
//!
//! Each test uses `tempfile::tempdir()` for isolation. All test fns return
//! `Result<(), String>` so we can use `?` instead of `.unwrap()` (which is
//! denied by the crate's clippy config).

use std::fs;

use openpgp::keyring::Keyring;
use openpgp::state::{KeyringEntry, KeyringIndex};
use tempfile::tempdir;

/// Build a `KeyringEntry` with sensible defaults for tests.
fn make_entry(fp: &str, uid: &str, has_secret: bool) -> KeyringEntry {
    KeyringEntry {
        fp: fp.to_string(),
        uid: uid.to_string(),
        has_secret,
        created: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn load_index_empty_dir() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());
    let idx = kr.load_index()?;
    assert!(idx.keys.is_empty(), "fresh dir should yield empty index");
    Ok(())
}

#[test]
fn load_index_roundtrip() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    // Manually write an index.json with one entry, then load it.
    let index = KeyringIndex {
        keys: vec![make_entry("ABCD1234EF567890", "alice@example.com", false)],
    };
    let json =
        serde_json::to_string_pretty(&index).map_err(|e| format!("serialize: {e}"))?;
    fs::write(kr.base_dir.join("index.json"), json)
        .map_err(|e| format!("write index.json: {e}"))?;

    let loaded = kr.load_index()?;
    assert_eq!(loaded.keys.len(), 1);
    let first = loaded
        .keys
        .first()
        .ok_or_else(|| "expected one entry".to_string())?;
    assert_eq!(first.fp, "ABCD1234EF567890");
    assert_eq!(first.uid, "alice@example.com");
    assert!(!first.has_secret);
    Ok(())
}

#[test]
fn upsert_entry_insert() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    kr.upsert_entry(make_entry("ABCD1234EF567890", "alice@example.com", false))?;

    let loaded = kr.load_index()?;
    assert_eq!(loaded.keys.len(), 1);
    let first = loaded
        .keys
        .first()
        .ok_or_else(|| "expected one entry".to_string())?;
    assert_eq!(first.fp, "ABCD1234EF567890");

    // index.json must exist on disk after upsert.
    assert!(
        kr.base_dir.join("index.json").exists(),
        "index.json should exist after upsert"
    );
    Ok(())
}

#[test]
fn upsert_entry_merge_by_fp() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    kr.upsert_entry(make_entry("ABC", "alice@example.com", false))?;
    // Same fp, different uid/has_secret — must replace, not append.
    kr.upsert_entry(make_entry("ABC", "bob@example.com", true))?;

    let loaded = kr.load_index()?;
    assert_eq!(loaded.keys.len(), 1, "should merge by fp, not duplicate");
    let first = loaded
        .keys
        .first()
        .ok_or_else(|| "expected one entry".to_string())?;
    assert_eq!(first.uid, "bob@example.com");
    assert!(first.has_secret, "has_secret should be updated to true");
    Ok(())
}

#[test]
fn remove_entry_deletes_files_and_index() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    let fp = "ABCD1234EF567890";
    kr.upsert_entry(make_entry(fp, "alice@example.com", true))?;
    kr.write_key_file(fp, false, "pub content")?;
    kr.write_key_file(fp, true, "sec content")?;

    let pub_path = kr.base_dir.join("keys").join(format!("{fp}.pub.asc"));
    let sec_path = kr.base_dir.join("keys").join(format!("{fp}.sec.asc"));
    assert!(pub_path.exists(), "pub key file should exist before remove");
    assert!(sec_path.exists(), "sec key file should exist before remove");

    kr.remove_entry(fp)?;

    let loaded = kr.load_index()?;
    assert!(
        loaded.keys.iter().all(|e| e.fp != fp),
        "index should not contain removed entry"
    );
    assert!(!pub_path.exists(), "pub key file should be deleted");
    assert!(!sec_path.exists(), "sec key file should be deleted");
    Ok(())
}

#[test]
fn remove_entry_nonexistent() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    // Idempotent: removing an entry that doesn't exist must NOT error.
    kr.remove_entry("NONEXISTENT")?;
    Ok(())
}

#[test]
fn write_key_file_pub() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    let fp = "ABCD1234EF567890";
    let content = "-----BEGIN PGP PUBLIC KEY BLOCK-----\narmored content\n-----END PGP PUBLIC KEY BLOCK-----\n";
    kr.write_key_file(fp, false, content)?;

    let path = kr.base_dir.join("keys").join(format!("{fp}.pub.asc"));
    assert!(path.exists(), "pub key file should exist");
    let read = fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    assert_eq!(read, content);
    Ok(())
}

#[test]
fn write_key_file_sec_permissions() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    let fp = "ABCD1234EF567890";
    let content = "-----BEGIN PGP PRIVATE KEY BLOCK-----\nsecret content\n-----END PGP PRIVATE KEY BLOCK-----\n";
    kr.write_key_file(fp, true, content)?;

    let path = kr.base_dir.join("keys").join(format!("{fp}.sec.asc"));
    assert!(path.exists(), "sec key file should exist");
    let read = fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
    assert_eq!(read, content);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&path).map_err(|e| format!("metadata: {e}"))?;
        let mode = metadata.permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "secret key file should have 0600 perms, got {:o}",
            mode
        );
    }
    Ok(())
}

#[test]
fn read_key_file_roundtrip() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    let fp = "ABCD1234EF567890";
    let content = "armored public key content";
    kr.write_key_file(fp, false, content)?;

    let read = kr.read_key_file(fp, false)?;
    assert_eq!(read, content);
    Ok(())
}

#[test]
fn read_key_file_missing() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    let result = kr.read_key_file("NONEXISTENT", false);
    assert!(result.is_err(), "reading a missing key file should error");
    Ok(())
}

#[test]
fn find_entry() -> Result<(), String> {
    let dir = tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let kr = Keyring::new(dir.path().to_path_buf());

    kr.upsert_entry(make_entry("FP1", "alice@example.com", false))?;
    kr.upsert_entry(make_entry("FP2", "bob@example.com", true))?;

    let found = kr.find_entry("FP1")?;
    let entry = found.ok_or_else(|| "expected Some(FP1)".to_string())?;
    assert_eq!(entry.fp, "FP1");
    assert_eq!(entry.uid, "alice@example.com");

    let not_found = kr.find_entry("NONEXISTENT")?;
    assert!(not_found.is_none(), "find_entry on missing fp should be None");
    Ok(())
}
