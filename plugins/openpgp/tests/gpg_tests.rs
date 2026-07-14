//! Integration tests for the `gpg.*` command family.
//!
//! All tests that touch a real keyring are guarded by `gpg_available()`
//! and serialized via `GPG_LOCK` because `GNUPGHOME` is a process-global
//! env var (cargo runs tests in parallel by default).

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use openpgp::codec::DyylValue;
use openpgp::commands;
use openpgp::state::PluginState;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

/// Process-wide lock so tests that mutate `GNUPGHOME` don't race.
/// Poisoning is recovered (rather than cascaded) so one test's panic
/// doesn't mask the real failures in the rest.
static GPG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> std::sync::MutexGuard<'static, ()> {
    GPG_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// True iff a usable `gpg` binary is on PATH.
fn gpg_available() -> bool {
    std::process::Command::new("gpg")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn str_arg(s: &str) -> DyylValue {
    DyylValue::Str(s.to_string())
}

/// Create an isolated `GNUPGHOME` under a fresh tempdir, set the env
/// var, and return a `PluginState` plus the tempdir (must be kept alive
/// for the test's duration). Caller must hold `lock()`.
fn make_gpg_state() -> (PluginState, tempfile::TempDir) {
    let dir = tempdir().expect("tempdir");
    let gnupghome = dir.path().join(".gnupg");
    std::fs::create_dir_all(&gnupghome).expect("create gnupghome");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&gnupghome, std::fs::Permissions::from_mode(0o700));
    }
    std::env::set_var("GNUPGHOME", &gnupghome);

    let state = PluginState {
        credentials_dir: PathBuf::from(dir.path()),
        ..Default::default()
    };
    (state, dir)
}

/// Generate a no-passphrase key in the current `GNUPGHOME`.
fn quick_gen_key(uid: &str, usage: &str) {
    let _ = std::process::Command::new("gpg")
        .args([
            "--batch",
            "--passphrase",
            "",
            "--quick-generate-key",
            uid,
            "rsa2048",
            usage,
            "0",
        ])
        .output();
}

fn dict_get<'a>(pairs: &'a [(DyylValue, DyylValue)], key: &str) -> &'a str {
    for (k, v) in pairs {
        if matches!(k, DyylValue::Str(s) if s == key) {
            if let DyylValue::Str(s) = v {
                return s.as_str();
            }
        }
    }
    ""
}

#[test]
fn gpg_detect() {
    let _g = lock();
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "gpg.detect", &[]).unwrap();
    if let DyylValue::Dict(pairs) = result {
        let installed = dict_get(&pairs, "installed");
        if gpg_available() {
            assert_eq!(installed, "1");
            // path and version should be non-empty when installed
            assert!(!dict_get(&pairs, "path").is_empty());
            assert!(!dict_get(&pairs, "version").is_empty());
        } else {
            assert_eq!(installed, "0");
        }
    } else {
        panic!("expected dict");
    }
}

#[test]
fn gpg_detect_when_not_installed_returns_zero() {
    // This test documents the not-installed branch; it only asserts the
    // shape when gpg is missing. When gpg IS installed, detect returns "1"
    // (covered by gpg_detect), so this test is a no-op then.
    if gpg_available() {
        return;
    }
    let _g = lock();
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "gpg.detect", &[]).unwrap();
    if let DyylValue::Dict(pairs) = result {
        assert_eq!(dict_get(&pairs, "installed"), "0");
    } else {
        panic!("expected dict");
    }
}

#[test]
fn gpg_exec_version() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "gpg.exec", &[str_arg("--version")]).unwrap();
    if let DyylValue::Str(s) = result {
        assert!(s.contains("gpg") || s.contains("GnuPG"));
    } else {
        panic!("expected string");
    }
}

#[test]
fn gpg_exec_with_list_args() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let mut state = PluginState::default();
    let result = commands::dispatch(
        &mut state,
        "gpg.exec",
        &[DyylValue::List(vec![str_arg("--version")])],
    )
    .unwrap();
    if let DyylValue::Str(s) = result {
        assert!(s.contains("gpg") || s.contains("GnuPG"));
    } else {
        panic!("expected string");
    }
}

#[test]
fn gpg_encrypt_decrypt_roundtrip() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let (mut state, _dir) = make_gpg_state();
    quick_gen_key("test@example.com", "encrypt");

    // Encrypt
    let result = commands::dispatch(
        &mut state,
        "gpg.encrypt",
        &[str_arg("hello world"), str_arg("test@example.com")],
    )
    .unwrap();
    let ciphertext = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert!(ciphertext.contains("BEGIN PGP MESSAGE"));

    // Decrypt
    let result = commands::dispatch(&mut state, "gpg.decrypt", &[str_arg(&ciphertext)]).unwrap();
    let plaintext = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert_eq!(plaintext, "hello world");
}

#[test]
fn gpg_encrypt_decrypt_file_roundtrip() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let (state, dir) = make_gpg_state();
    let mut state = state;
    quick_gen_key("filetest@example.com", "encrypt");

    let in_path = dir.path().join("plain.txt");
    let out_path = dir.path().join("plain.gpg");
    let dec_path = dir.path().join("plain.dec");
    std::fs::write(&in_path, "file body content").unwrap();

    let result = commands::dispatch(
        &mut state,
        "gpg.encrypt.file",
        &[
            str_arg(in_path.to_str().unwrap()),
            str_arg(out_path.to_str().unwrap()),
            str_arg("filetest@example.com"),
        ],
    )
    .unwrap();
    assert_eq!(as_str(&result), "1");
    assert!(out_path.exists());

    let result = commands::dispatch(
        &mut state,
        "gpg.decrypt.file",
        &[
            str_arg(out_path.to_str().unwrap()),
            str_arg(dec_path.to_str().unwrap()),
        ],
    )
    .unwrap();
    assert_eq!(as_str(&result), "1");
    assert_eq!(std::fs::read_to_string(&dec_path).unwrap(), "file body content");
}

#[test]
fn gpg_sign_verify_detached_roundtrip() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let (mut state, _dir) = make_gpg_state();
    quick_gen_key("signer@example.com", "sign");

    // Sign (detached)
    let result = commands::dispatch(
        &mut state,
        "gpg.sign",
        &[str_arg("hello"), str_arg("signer@example.com"), str_arg("1")],
    )
    .unwrap();
    let sig = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert!(sig.contains("BEGIN PGP SIGNATURE"));

    // Verify (detached)
    let result = commands::dispatch(
        &mut state,
        "gpg.verify",
        &[str_arg(&sig), str_arg("hello")],
    )
    .unwrap();
    if let DyylValue::Dict(pairs) = result {
        assert_eq!(dict_get(&pairs, "valid"), "1");
        assert!(dict_get(&pairs, "signer").contains("signer@example.com"));
    } else {
        panic!("expected dict");
    }
}

#[test]
fn gpg_sign_verify_file_detached_roundtrip() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let (state, dir) = make_gpg_state();
    let mut state = state;
    quick_gen_key("filesigner@example.com", "sign");

    let data_path = dir.path().join("data.txt");
    let sig_path = dir.path().join("data.sig");
    std::fs::write(&data_path, "file body to sign").unwrap();

    let result = commands::dispatch(
        &mut state,
        "gpg.sign.file",
        &[
            str_arg(data_path.to_str().unwrap()),
            str_arg(sig_path.to_str().unwrap()),
            str_arg("filesigner@example.com"),
            str_arg("1"),
        ],
    )
    .unwrap();
    assert_eq!(as_str(&result), "1");
    assert!(sig_path.exists());

    let result = commands::dispatch(
        &mut state,
        "gpg.verify.file",
        &[
            str_arg(sig_path.to_str().unwrap()),
            str_arg(data_path.to_str().unwrap()),
        ],
    )
    .unwrap();
    if let DyylValue::Dict(pairs) = result {
        assert_eq!(dict_get(&pairs, "valid"), "1");
        assert!(dict_get(&pairs, "signer").contains("filesigner@example.com"));
    } else {
        panic!("expected dict");
    }
}

#[test]
fn gpg_key_import_export_list_roundtrip() {
    let _g = lock();
    if !gpg_available() {
        return;
    }
    let (state, dir) = make_gpg_state();
    let mut state = state;
    quick_gen_key("impex@example.com", "default");

    // Export the public key
    let result = commands::dispatch(
        &mut state,
        "gpg.key.export",
        &[str_arg("impex@example.com"), str_arg("0")],
    )
    .unwrap();
    let exported = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert!(exported.contains("BEGIN PGP PUBLIC KEY BLOCK"));

    // Wipe the keyring and re-import into a fresh GNUPGHOME
    let dir2 = tempdir().expect("tempdir2");
    let gnupghome2 = dir2.path().join(".gnupg");
    std::fs::create_dir_all(&gnupghome2).expect("create gnupghome2");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&gnupghome2, std::fs::Permissions::from_mode(0o700));
    }
    std::env::set_var("GNUPGHOME", &gnupghome2);

    let result = commands::dispatch(
        &mut state,
        "gpg.key.import",
        &[str_arg(&exported)],
    )
    .unwrap();
    let count = as_str(&result);
    // count should be a non-empty numeric string
    assert!(!count.is_empty());
    assert!(count.chars().all(|c| c.is_ascii_digit()));

    // List keys — should contain the imported key
    let result = commands::dispatch(&mut state, "gpg.key.list", &[]).unwrap();
    if let DyylValue::List(items) = result {
        let mut found = false;
        for item in &items {
            if let DyylValue::Dict(pairs) = item {
                let uid = dict_get(pairs, "uid");
                if uid.contains("impex@example.com") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "imported key should appear in gpg.key.list");
    } else {
        panic!("expected list");
    }
    // keep dir2 alive
    drop(dir2);
    let _ = dir;
}

fn as_str(v: &DyylValue) -> &str {
    match v {
        DyylValue::Str(s) => s.as_str(),
        _ => panic!("expected string"),
    }
}
