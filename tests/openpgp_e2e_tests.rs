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
//! End-to-end tests for the OpenPGP plugin via the full dyyl runtime.
//!
//! These tests build the plugin via `build-openpgp.sh`, install it to a
//! temp XDG_DATA_HOME, create a credentials.toml in a temp XDG_CONFIG_HOME,
//! then run the dyyl binary against golden fixture scripts that exercise
//! the plugin's key.generate / encrypt / decrypt / sign / verify /
//! sym.encrypt / sym.decrypt / gpg.detect / key.list commands.
//!
//! Unlike `openpgp_plugin_tests.rs` (which dlopens the .so directly), these
//! tests go through the full dyyl CLI → parser → dispatcher → plugin
//! manager → dlopen → handle_command pipeline.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Path to the compiled dyyl binary, resolved by `cargo test`.
fn dyyl_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dyyl"))
}

/// Path to the repository root (where `tests/fixtures/` lives).
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Build the OpenPGP plugin via `build-openpgp.sh` and return the path to
/// the built shared library. The temp dir holding the build output is
/// leaked so the .so persists for the test duration.
fn build_plugin() -> PathBuf {
    let temp = tempfile::tempdir().expect("create tempdir for plugin build");
    let output_dir = temp.path().to_path_buf();

    let script = repo_root()
        .join("tests")
        .join("fixtures")
        .join("build-openpgp.sh");

    let status = Command::new("bash")
        .arg(&script)
        .arg(&output_dir)
        .status()
        .expect("failed to run build-openpgp.sh");

    assert!(status.success(), "build-openpgp.sh failed");

    let so = output_dir.join("libopenpgp.so");
    let dylib = output_dir.join("libopenpgp.dylib");
    let dll = output_dir.join("openpgp.dll");

    let lib_path = if so.exists() {
        so
    } else if dylib.exists() {
        dylib
    } else if dll.exists() {
        dll
    } else {
        panic!("no built plugin library found in {}", output_dir.display());
    };

    // Leak the tempdir so the .so survives for the test.
    std::mem::forget(temp);
    lib_path
}

/// Read `plugin.toml.in` and append the `[installed]` section required by
/// `LocalPluginToml` so it can be deserialized by the plugin manager.
fn build_plugin_toml() -> String {
    let toml_in = repo_root()
        .join("plugins")
        .join("openpgp")
        .join("plugin.toml.in");

    let content =
        fs::read_to_string(&toml_in).unwrap_or_else(|e| panic!("read plugin.toml.in: {e}"));

    // Append the [installed] section with dummy metadata.
    format!(
        "{content}\n\
         [installed]\n\
         source_url = \"local\"\n\
         sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n\
         installed_at = \"2026-01-01T00:00:00Z\"\n\
         dyyl_version = \"{version}\"\n",
        version = env!("CARGO_PKG_VERSION")
    )
}

/// Install the plugin into a temp XDG_DATA_HOME.
///
/// Creates `<xdg_data>/dyyl/plugins/openpgp/0.1.0/libopenpgp.so` and
/// `plugin.toml`. Returns the temp dir (must be held alive for the test).
fn install_plugin() -> TempDir {
    let xdg_data = tempfile::tempdir().expect("create xdg_data tempdir");

    // Build the plugin.
    let lib_path = build_plugin();

    // Install directory: <xdg_data>/dyyl/plugins/openpgp/0.1.0/
    let version_dir = xdg_data
        .path()
        .join("dyyl")
        .join("plugins")
        .join("openpgp")
        .join("0.1.0");

    fs::create_dir_all(&version_dir).unwrap_or_else(|e| panic!("create version_dir: {e}"));

    // Copy the shared library.
    let lib_filename = lib_path.file_name().expect("lib filename");
    let dest_lib = version_dir.join(lib_filename);
    fs::copy(&lib_path, &dest_lib).unwrap_or_else(|e| panic!("copy plugin lib: {e}"));

    // Write plugin.toml (from .in + [installed] section).
    let toml_content = build_plugin_toml();
    let dest_toml = version_dir.join("plugin.toml");
    fs::write(&dest_toml, &toml_content).unwrap_or_else(|e| panic!("write plugin.toml: {e}"));

    xdg_data
}

/// Create a credentials.toml in a temp XDG_CONFIG_HOME.
///
/// The `[plugin.openpgp]` section provides `passphrase` and `default_key`.
/// The `__credentials_dir` is auto-injected by the plugin manager.
fn make_credentials_config() -> TempDir {
    let xdg_config = tempfile::tempdir().expect("create xdg_config tempdir");

    let dyyl_config_dir = xdg_config.path().join("dyyl");
    fs::create_dir_all(&dyyl_config_dir).unwrap_or_else(|e| panic!("create config dir: {e}"));

    let creds_path = dyyl_config_dir.join("credentials.toml");
    fs::write(
        &creds_path,
        "[plugin.openpgp]\n\
         passphrase = \"test-pass\"\n\
         default_key = \"\"\n",
    )
    .unwrap_or_else(|e| panic!("write credentials.toml: {e}"));

    xdg_config
}

/// Run the dyyl binary against a fixture with XDG env vars set.
/// Returns (exit_code, stdout, stderr).
fn run_dyyl_with_plugin(
    fixture: &str,
    xdg_data: &Path,
    xdg_config: &Path,
) -> (i32, String, String) {
    let bin = dyyl_bin();
    let fixture_path = repo_root().join("tests").join("fixtures").join(fixture);

    let output = Command::new(&bin)
        .arg(&fixture_path)
        .env("XDG_DATA_HOME", xdg_data)
        .env("XDG_CONFIG_HOME", xdg_config)
        .output()
        .expect("failed to execute dyyl binary");

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    (code, stdout, stderr)
}

// ── Test: key.generate → encrypt → decrypt ──────────────────────────────

#[test]
fn e2e_openpgp_roundtrip() {
    let xdg_data = install_plugin();
    let xdg_config = make_credentials_config();

    let (code, stdout, stderr) =
        run_dyyl_with_plugin("openpgp-roundtrip.dyyl", xdg_data.path(), xdg_config.path());

    assert_eq!(
        code, 0,
        "exit code must be 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The fingerprint (40 hex chars) should be on the first line.
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 3,
        "expected at least 3 output lines, got {}\nstdout: {stdout}",
        lines.len()
    );
    let fp = lines[0];
    assert_eq!(
        fp.len(),
        40,
        "fingerprint should be 40 hex chars, got '{fp}'"
    );
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint should be hex, got '{fp}'"
    );

    // The ciphertext should be an armored PGP message.
    assert!(
        stdout.contains("-----BEGIN PGP MESSAGE-----"),
        "stdout should contain PGP message armor\nstdout: {stdout}"
    );

    // The decrypted plaintext should be "secret message".
    let last_line = lines[lines.len() - 1];
    assert_eq!(
        last_line, "secret message",
        "decrypted text should be 'secret message', got '{last_line}'\nstdout: {stdout}"
    );
}

// ── Test: sign detached → verify ─────────────────────────────────────────

#[test]
fn e2e_openpgp_sign_verify() {
    let xdg_data = install_plugin();
    let xdg_config = make_credentials_config();

    let (code, stdout, stderr) = run_dyyl_with_plugin(
        "openpgp-sign-verify.dyyl",
        xdg_data.path(),
        xdg_config.path(),
    );

    assert_eq!(
        code, 0,
        "exit code must be 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The script prints dict.get($result, valid) which should be "1".
    let trimmed = stdout.trim();
    assert_eq!(
        trimmed, "1",
        "verify result 'valid' should be '1' (valid signature), got '{trimmed}'\nstdout: {stdout}"
    );
}

// ── Test: symmetric encrypt / decrypt ───────────────────────────────────

#[test]
fn e2e_openpgp_sym() {
    let xdg_data = install_plugin();
    let xdg_config = make_credentials_config();

    let (code, stdout, stderr) =
        run_dyyl_with_plugin("openpgp-sym.dyyl", xdg_data.path(), xdg_config.path());

    assert_eq!(
        code, 0,
        "exit code must be 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The ciphertext should be an armored PGP message.
    assert!(
        stdout.contains("-----BEGIN PGP MESSAGE-----"),
        "stdout should contain PGP message armor\nstdout: {stdout}"
    );

    // The decrypted plaintext should be "secret data".
    let last_line = stdout.lines().last().expect("at least one output line");
    assert_eq!(
        last_line, "secret data",
        "decrypted text should be 'secret data', got '{last_line}'\nstdout: {stdout}"
    );
}

// ── Test: gpg.detect ────────────────────────────────────────────────────

#[test]
fn e2e_openpgp_gpg_detect() {
    let xdg_data = install_plugin();
    let xdg_config = make_credentials_config();

    let (code, stdout, stderr) = run_dyyl_with_plugin(
        "openpgp-gpg-detect.dyyl",
        xdg_data.path(),
        xdg_config.path(),
    );

    assert_eq!(
        code, 0,
        "exit code must be 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The script prints dict.get($r, installed) which should be "0" or "1".
    let trimmed = stdout.trim();
    assert!(
        trimmed == "0" || trimmed == "1",
        "gpg.detect 'installed' should be '0' or '1', got '{trimmed}'\nstdout: {stdout}"
    );
}

// ── Test: key.generate → key.list persistence ──────────────────────────

#[test]
fn e2e_openpgp_keyring_persist() {
    let xdg_data = install_plugin();
    let xdg_config = make_credentials_config();

    let (code, stdout, stderr) = run_dyyl_with_plugin(
        "openpgp-keyring-persist.dyyl",
        xdg_data.path(),
        xdg_config.path(),
    );

    assert_eq!(
        code, 0,
        "exit code must be 0\nstdout: {stdout}\nstderr: {stderr}"
    );

    // The script prints the generated fingerprint, then the fingerprint
    // found via key.list → list.get(0) → dict.get(fp). They should match.
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 2,
        "expected at least 2 output lines, got {}\nstdout: {stdout}",
        lines.len()
    );

    let generated_fp = lines[0];
    let found_fp = lines[1];

    assert_eq!(
        generated_fp.len(),
        40,
        "generated fingerprint should be 40 hex chars, got '{generated_fp}'"
    );
    assert_eq!(
        generated_fp, found_fp,
        "key.list should contain the generated key's fingerprint\n\
         generated: {generated_fp}\n\
         found:     {found_fp}\n\
         stdout: {stdout}"
    );
}
