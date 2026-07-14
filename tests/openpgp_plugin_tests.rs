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
//! Integration tests for the OpenPGP plugin via raw dlopen (libloading).
//!
//! These tests build the plugin via tests/fixtures/build-openpgp.sh, then
//! load the .so directly and exercise the 15 ABI symbols.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::path::PathBuf;
use std::process::Command;

use libloading::{Library, Symbol};
use tempfile::TempDir;

/// Build the plugin and return the path to the built .so/.dylib/.dll.
fn build_plugin() -> PathBuf {
    let temp = tempfile::tempdir().expect("create tempdir for plugin build");
    let output_dir = temp.path();

    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("build-openpgp.sh");

    let status = Command::new("bash")
        .arg(&script)
        .arg(output_dir)
        .status()
        .expect("failed to run build-openpgp.sh");

    assert!(status.success(), "build-openpgp.sh failed");

    // Find the built library
    let so = output_dir.join("libopenpgp.so");
    let dylib = output_dir.join("libopenpgp.dylib");
    let dll = output_dir.join("openpgp.dll");

    if so.exists() {
        // Leak the tempdir — we need the .so to persist for the test
        std::mem::forget(temp);
        so
    } else if dylib.exists() {
        std::mem::forget(temp);
        dylib
    } else if dll.exists() {
        std::mem::forget(temp);
        dll
    } else {
        panic!("no built plugin library found in {}", output_dir.display());
    }
}

/// Create a temp directory for credentials and return (tempdir, creds_dir_path).
/// The tempdir must be held alive for the duration of the test.
fn make_creds_dir() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let creds_dir = dir
        .path()
        .join("dyyl")
        .join("credentials.d")
        .join("openpgp");
    std::fs::create_dir_all(&creds_dir).expect("create creds dir");
    (dir, creds_dir)
}

/// Extract a string from a *mut c_char (allocated by the plugin).
fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn test_abi_load_and_resolve_all_symbols() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    // Resolve all 15 symbols
    unsafe {
        let _get_api_version: Symbol<unsafe extern "C" fn() -> c_uint> = lib
            .get(b"dyyl_plugin_get_api_version")
            .expect("get_api_version");
        let _get_name: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_get_name").expect("get_name");
        let _get_version: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_get_version").expect("get_version");
        let _get_author: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_get_author").expect("get_author");
        let _get_description: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> = lib
            .get(b"dyyl_plugin_get_description")
            .expect("get_description");
        let _init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let _on_load: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            lib.get(b"dyyl_plugin_on_load").expect("on_load");
        let _list_commands: Symbol<unsafe extern "C" fn(*mut c_void, *mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_list_commands")
                .expect("list_commands");
        let _get_command_help: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_char) -> c_int,
        > = lib
            .get(b"dyyl_plugin_get_command_help")
            .expect("get_command_help");
        let _handle_command: Symbol<
            unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *mut *mut c_char,
            ) -> c_int,
        > = lib
            .get(b"dyyl_plugin_handle_command")
            .expect("handle_command");
        let _on_error: Symbol<
            unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *const c_char) -> c_int,
        > = lib.get(b"dyyl_plugin_on_error").expect("on_error");
        let _on_unload: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            lib.get(b"dyyl_plugin_on_unload").expect("on_unload");
        let _shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");
        let _free_string: Symbol<unsafe extern "C" fn(*mut c_char)> =
            lib.get(b"dyyl_plugin_free_string").expect("free_string");
        let _set_credentials: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int> =
            lib.get(b"dyyl_plugin_set_credentials")
                .expect("set_credentials");
    }

    println!("All 15 ABI symbols resolved successfully");
}

#[test]
fn test_get_api_version_returns_2() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let get_api_version: Symbol<unsafe extern "C" fn() -> c_uint> = lib
            .get(b"dyyl_plugin_get_api_version")
            .expect("get_api_version");
        let version = get_api_version();
        assert_eq!(version, 2, "API version should be 2");
    }
}

#[test]
fn test_get_name_and_version() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let get_name: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_get_name").expect("get_name");
        let get_version: Symbol<unsafe extern "C" fn(*mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_get_version").expect("get_version");
        let free_string: Symbol<unsafe extern "C" fn(*mut c_char)> =
            lib.get(b"dyyl_plugin_free_string").expect("free_string");

        let mut name_ptr: *mut c_char = std::ptr::null_mut();
        let rc = get_name(&mut name_ptr);
        assert_eq!(rc, 0);
        let name = cstr_to_string(name_ptr);
        assert_eq!(name, "openpgp");
        free_string(name_ptr);

        let mut version_ptr: *mut c_char = std::ptr::null_mut();
        let rc = get_version(&mut version_ptr);
        assert_eq!(rc, 0);
        let version = cstr_to_string(version_ptr);
        assert_eq!(version, "0.1.0");
        free_string(version_ptr);
    }
}

#[test]
fn test_init_returns_nonnull_handle() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");

        let handle = init(2);
        assert!(!handle.is_null(), "init should return non-null handle");
        shutdown(handle);
    }
}

#[test]
fn test_set_credentials_and_on_load() {
    let lib_path = build_plugin();
    let (_creds_temp, creds_dir) = make_creds_dir();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let set_credentials: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int> =
            lib.get(b"dyyl_plugin_set_credentials")
                .expect("set_credentials");
        let on_load: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            lib.get(b"dyyl_plugin_on_load").expect("on_load");
        let shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");

        let handle = init(2);
        assert!(!handle.is_null());

        // Set credentials with test JSON pointing to an isolated temp dir
        let creds_json = format!(
            r#"{{"passphrase":"test-pass","default_key":"","__credentials_dir":"{}"}}"#,
            creds_dir.display()
        );
        let creds_c = CString::new(creds_json).unwrap();
        let rc = set_credentials(handle, creds_c.as_ptr());
        assert_eq!(rc, 0, "set_credentials should return 0");

        let rc = on_load(handle);
        assert_eq!(rc, 0, "on_load should return 0");

        shutdown(handle);
    }
}

#[test]
fn test_handle_command_key_generate() {
    let lib_path = build_plugin();
    let (_creds_temp, creds_dir) = make_creds_dir();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let set_credentials: Symbol<unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int> =
            lib.get(b"dyyl_plugin_set_credentials")
                .expect("set_credentials");
        let on_load: Symbol<unsafe extern "C" fn(*mut c_void) -> c_int> =
            lib.get(b"dyyl_plugin_on_load").expect("on_load");
        let handle_command: Symbol<
            unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *mut *mut c_char,
            ) -> c_int,
        > = lib
            .get(b"dyyl_plugin_handle_command")
            .expect("handle_command");
        let free_string: Symbol<unsafe extern "C" fn(*mut c_char)> =
            lib.get(b"dyyl_plugin_free_string").expect("free_string");
        let shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");

        let handle = init(2);
        assert!(!handle.is_null());

        // Set credentials — __credentials_dir points to our isolated temp dir
        let creds_json = format!(
            r#"{{"passphrase":"test-pass","default_key":"","__credentials_dir":"{}"}}"#,
            creds_dir.display()
        );
        let creds_c = CString::new(creds_json).unwrap();
        let rc = set_credentials(handle, creds_c.as_ptr());
        assert_eq!(rc, 0);

        let rc = on_load(handle);
        assert_eq!(rc, 0);

        // Call key.generate
        let cmd = CString::new("key.generate").unwrap();
        let args = r#"[{"type":"str","value":"test <test@example.com>"},{"type":"str","value":"test-pass"}]"#;
        let args_c = CString::new(args).unwrap();
        let mut out_ptr: *mut c_char = std::ptr::null_mut();

        let rc = handle_command(handle, cmd.as_ptr(), args_c.as_ptr(), &mut out_ptr);
        assert_eq!(rc, 0, "key.generate should return 0 (success)");

        let result_json = cstr_to_string(out_ptr);
        free_string(out_ptr);

        // Result should be {"type":"str","value":"<40-char hex fingerprint>"}
        assert!(
            result_json.contains(r#""type":"str""#),
            "result should be a string value, got: {result_json}"
        );
        assert!(
            result_json.contains(r#""value":"#),
            "result should have a value field"
        );

        // Extract the fingerprint from the JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&result_json).expect("parse result json");
        let fp = parsed
            .get("value")
            .and_then(|v| v.as_str())
            .expect("extract fingerprint");
        assert_eq!(
            fp.len(),
            40,
            "fingerprint should be 40 hex chars, got: {fp}"
        );
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint should be hex"
        );

        shutdown(handle);
    }
}

#[test]
fn test_list_commands_returns_30_commands() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let list_commands: Symbol<unsafe extern "C" fn(*mut c_void, *mut *mut c_char) -> c_int> =
            lib.get(b"dyyl_plugin_list_commands")
                .expect("list_commands");
        let free_string: Symbol<unsafe extern "C" fn(*mut c_char)> =
            lib.get(b"dyyl_plugin_free_string").expect("free_string");
        let shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");

        let handle = init(2);
        assert!(!handle.is_null());

        let mut out_ptr: *mut c_char = std::ptr::null_mut();
        let rc = list_commands(handle, &mut out_ptr);
        assert_eq!(rc, 0);

        let json = cstr_to_string(out_ptr);
        free_string(out_ptr);

        let commands: Vec<serde_json::Value> =
            serde_json::from_str(&json).expect("parse command list");
        assert_eq!(
            commands.len(),
            30,
            "should have 30 commands, got: {commands:?}"
        );

        // Verify some command names
        let names: Vec<String> = commands
            .iter()
            .filter_map(|c| c.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect();
        assert!(
            names.contains(&"key.generate".to_string()),
            "should have key.generate"
        );
        assert!(
            names.contains(&"gpg.detect".to_string()),
            "should have gpg.detect"
        );
        assert!(
            names.contains(&"encrypt".to_string()),
            "should have encrypt"
        );
        assert!(names.contains(&"armor".to_string()), "should have armor");

        shutdown(handle);
    }
}

#[test]
fn test_handle_command_unknown_returns_error() {
    let lib_path = build_plugin();

    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        let init: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        let handle_command: Symbol<
            unsafe extern "C" fn(
                *mut c_void,
                *const c_char,
                *const c_char,
                *mut *mut c_char,
            ) -> c_int,
        > = lib
            .get(b"dyyl_plugin_handle_command")
            .expect("handle_command");
        let free_string: Symbol<unsafe extern "C" fn(*mut c_char)> =
            lib.get(b"dyyl_plugin_free_string").expect("free_string");
        let shutdown: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"dyyl_plugin_shutdown").expect("shutdown");

        let handle = init(2);

        let cmd = CString::new("nonexistent.command").unwrap();
        let args_c = CString::new("[]").unwrap();
        let mut out_ptr: *mut c_char = std::ptr::null_mut();

        let rc = handle_command(handle, cmd.as_ptr(), args_c.as_ptr(), &mut out_ptr);
        assert_eq!(rc, 1, "unknown command should return 1 (error)");

        let error_json = cstr_to_string(out_ptr);
        free_string(out_ptr);

        assert!(
            error_json.contains(r#""code""#),
            "error should have code field"
        );
        assert!(
            error_json.contains(r#""unknown_command""#),
            "error code should be unknown_command"
        );

        shutdown(handle);
    }
}
