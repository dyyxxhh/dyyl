//! Integration tests for the `armor` and `dearmor` commands.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use openpgp::codec::DyylValue;
use openpgp::commands;
use openpgp::state::PluginState;

/// Wrap a `&str` as a `DyylValue::Str` arg.
fn str_arg(s: &str) -> DyylValue {
    DyylValue::Str(s.to_string())
}

#[test]
fn armor_dearmor_roundtrip() {
    let mut state = PluginState::default();
    // "hello world" in base64
    let original_b64 = "aGVsbG8gd29ybGQ=";

    let result = commands::dispatch(&mut state, "armor", &[str_arg(original_b64)]).unwrap();
    let armored = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert!(armored.contains("BEGIN PGP"));

    let result = commands::dispatch(&mut state, "dearmor", &[str_arg(&armored)]).unwrap();
    let b64 = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert_eq!(b64, original_b64);
}

#[test]
fn armor_provides_pgp_headers() {
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "armor", &[str_arg("dGVzdA==")]).unwrap();
    let armored = match result {
        DyylValue::Str(s) => s,
        _ => panic!("expected string"),
    };
    assert!(armored.contains("BEGIN PGP"));
    assert!(armored.contains("END PGP"));
}

#[test]
fn armor_rejects_non_string_arg() {
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "armor", &[DyylValue::Empty]);
    assert!(result.is_err());
}

#[test]
fn dearmor_rejects_non_string_arg() {
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "dearmor", &[DyylValue::Empty]);
    assert!(result.is_err());
}

#[test]
fn armor_rejects_invalid_base64() {
    let mut state = PluginState::default();
    let result = commands::dispatch(&mut state, "armor", &[str_arg("!!! not base64 !!!")]);
    assert!(result.is_err());
}
