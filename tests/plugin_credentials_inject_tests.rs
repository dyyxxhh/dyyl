#![allow(
    clippy::all,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::as_underscore,
    clippy::fn_to_numeric_cast_any,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use dyyl::i18n::Lang;
use dyyl::runtime::plugin::creds_inject::build_credentials_json;
use dyyl::runtime::plugin::manifest::{CredentialField, CredentialsSpec};

/// Serialize tests that mutate `XDG_DATA_HOME` so they don't race in parallel.
static XDG_LOCK: Mutex<()> = Mutex::new(());

fn with_temp_xdg<F: FnOnce(&std::path::PathBuf)>(test: F) {
    let _guard = XDG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("dyyl-creds-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    std::env::set_var("XDG_DATA_HOME", &tmp);
    test(&tmp);
    std::env::remove_var("XDG_DATA_HOME");
    let _ = fs::remove_dir_all(&tmp);
}

fn field(name: &str, ty: &str, secret: bool) -> CredentialField {
    CredentialField {
        name: name.to_string(),
        r#type: ty.to_string(),
        secret,
        description: String::new(),
    }
}

#[test]
fn string_field_resolved_from_toml() {
    with_temp_xdg(|_| {
        let spec = CredentialsSpec {
            fields: vec![field("token", "string", true)],
        };
        let mut toml = HashMap::new();
        toml.insert("token".to_string(), "ghp_abc".to_string());
        let json = build_credentials_json(Some(&spec), "testplugin", &toml, Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["token"], "ghp_abc");
        assert!(v["__credentials_dir"].is_string());
    });
}

#[test]
fn string_field_missing_returns_error() {
    with_temp_xdg(|_| {
        let spec = CredentialsSpec {
            fields: vec![field("token", "string", true)],
        };
        let toml = HashMap::new();
        assert!(build_credentials_json(Some(&spec), "testplugin", &toml, Lang::En).is_err());
    });
}

#[test]
fn file_field_reads_file_content() {
    with_temp_xdg(|xdg| {
        let creds_dir = xdg.join("dyyl/credentials.d/testplugin");
        fs::create_dir_all(&creds_dir).unwrap();
        fs::write(
            creds_dir.join("default_pubkey"),
            "-----BEGIN PGP PUBLIC KEY-----\n...",
        )
        .unwrap();
        let spec = CredentialsSpec {
            fields: vec![field("default_pubkey", "file", false)],
        };
        let toml = HashMap::new();
        let json = build_credentials_json(Some(&spec), "testplugin", &toml, Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["default_pubkey"]
            .as_str()
            .unwrap()
            .contains("BEGIN PGP PUBLIC KEY"));
    });
}

#[test]
fn file_field_missing_injects_empty_string() {
    with_temp_xdg(|_| {
        let spec = CredentialsSpec {
            fields: vec![field("nonexistent", "file", false)],
        };
        let toml = HashMap::new();
        let json = build_credentials_json(Some(&spec), "testplugin", &toml, Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["nonexistent"], "");
    });
}

#[test]
fn credentials_dir_auto_created_with_0700() {
    with_temp_xdg(|xdg| {
        let expected = xdg.join("dyyl/credentials.d/newplugin");
        assert!(!expected.exists());
        let _ = build_credentials_json(None, "newplugin", &HashMap::new(), Lang::En).unwrap();
        assert!(expected.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&expected).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o700);
        }
    });
}

#[test]
fn credentials_dir_auto_injected_even_without_spec() {
    with_temp_xdg(|_| {
        let json = build_credentials_json(None, "simpleplugin", &HashMap::new(), Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["__credentials_dir"]
            .as_str()
            .unwrap()
            .ends_with("credentials.d/simpleplugin"));
    });
}

#[test]
fn directory_field_returns_subdir_path() {
    with_temp_xdg(|_| {
        let spec = CredentialsSpec {
            fields: vec![field("keyring", "directory", false)],
        };
        let json =
            build_credentials_json(Some(&spec), "testplugin", &HashMap::new(), Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["keyring"]
            .as_str()
            .unwrap()
            .ends_with("credentials.d/testplugin/keyring"));
    });
}

#[test]
fn build_json_handles_empty_spec_and_empty_toml() {
    with_temp_xdg(|_| {
        let json = build_credentials_json(None, "p", &HashMap::new(), Lang::En).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 1);
        assert!(v["__credentials_dir"].is_string());
    });
}
