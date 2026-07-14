//! `gpg.*` commands — system gpg wrapper (13 commands).
//!
//! All commands in this module shell out to the system `gpg` binary.
//! They are independent of `PluginState` credentials/keyring — gpg uses
//! its own keyring (controlled by `GNUPGHOME`).

use std::io::Write as IoWrite;
use std::process::{Command, Stdio};

use shell_words::split as shell_split;
use which::which;

use crate::codec::DyylValue;
use crate::error::PluginError;
use crate::state::PluginState;

/// Find the gpg binary path. Returns `None` if not found on PATH.
fn gpg_path() -> Option<String> {
    which("gpg")
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Run gpg with `args`, optionally piping `stdin` to the child.
/// Returns `(stdout, stderr, exit_code)`.
fn run_gpg(args: &[&str], stdin: Option<&[u8]>) -> Result<(String, String, i32), PluginError> {
    let gpg = gpg_path().ok_or_else(|| {
        PluginError::gpg_not_installed("gpg binary not found in PATH")
    })?;

    let mut cmd = Command::new(&gpg);
    cmd.args(args);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| PluginError::gpg_exec_failed(format!("spawn gpg: {e}")))?;

    if let Some(data) = stdin {
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin
                .write_all(data)
                .map_err(|e| PluginError::gpg_exec_failed(format!("write stdin: {e}")))?;
        }
    }
    // Drop stdin to signal EOF.
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| PluginError::gpg_exec_failed(format!("wait gpg: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, code))
}

/// Run gpg and return stdout on success. Errors on non-zero exit.
fn run_gpg_or_fail(args: &[&str], stdin: Option<&[u8]>) -> Result<String, PluginError> {
    let (stdout, stderr, code) = run_gpg(args, stdin)?;
    if code != 0 {
        return Err(PluginError::gpg_exec_failed(format!(
            "gpg exit {code}: {stderr}"
        )));
    }
    Ok(stdout)
}

/// Build a `(Str(k), Str(v))` dict pair.
fn pair(k: &str, v: &str) -> (DyylValue, DyylValue) {
    (DyylValue::Str(k.to_string()), DyylValue::Str(v.to_string()))
}

/// Extract the signer UID from `gpg --verify` stderr output. gpg emits
/// a line like `gpg: Good signature from "Name <email>"`.
fn extract_signer_from_gpg_output(stderr: &str) -> String {
    for line in stderr.lines() {
        if line.contains("Good signature from") {
            if let Some((_, rest)) = line.split_once('"') {
                if let Some((name, _)) = rest.split_once('"') {
                    return name.to_string();
                }
            }
        }
    }
    String::new()
}

/// `gpg.detect` (arity 0): detect system gpg installation, return
/// `{installed, path, version}`. Never errors.
pub fn detect(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    match gpg_path() {
        None => Ok(DyylValue::Dict(vec![
            pair("installed", "0"),
            pair("path", ""),
            pair("version", ""),
        ])),
        Some(path) => {
            let (stdout, _stderr, code) = match run_gpg(&["--version"], None) {
                Ok(t) => t,
                Err(_) => (String::new(), String::new(), -1),
            };
            let version = if code == 0 {
                stdout.lines().next().unwrap_or("").to_string()
            } else {
                String::new()
            };
            Ok(DyylValue::Dict(vec![
                pair("installed", "1"),
                pair("path", &path),
                pair("version", &version),
            ]))
        }
    }
}

/// `gpg.exec` (arity 1+): execute raw gpg command with args string or
/// list. Returns stdout string; empty string on non-zero exit.
pub fn exec(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let gpg_args: Vec<String> = if let Some(first) = args.first() {
        match first {
            DyylValue::Str(s) => shell_split(s)
                .map_err(|e| PluginError::runtime(format!("shell split: {e}")))?,
            DyylValue::List(items) => items
                .iter()
                .filter_map(|i| i.as_str().map(String::from))
                .collect(),
            _ => return Err(PluginError::type_error("gpg.exec expects string or list arg")),
        }
    } else {
        return Err(PluginError::arity_mismatch("gpg.exec expects (args)"));
    };

    let arg_refs: Vec<&str> = gpg_args.iter().map(String::as_str).collect();
    let (stdout, stderr, code) = run_gpg(&arg_refs, None)?;

    if code != 0 {
        eprintln!("[openpgp] gpg exited {code}: {stderr}");
        return Ok(DyylValue::Str(String::new()));
    }
    Ok(DyylValue::Str(stdout))
}

/// `gpg.encrypt` (arity 2): encrypt text using system gpg for a recipient.
pub fn gpg_encrypt(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.encrypt expects (text, recipient)"))?;
    let recipient = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.encrypt expects (text, recipient)"))?;

    let stdout = run_gpg_or_fail(
        &["--armor", "--encrypt", "--recipient", recipient],
        Some(text.as_bytes()),
    )?;
    Ok(DyylValue::Str(stdout))
}

/// `gpg.encrypt.file` (arity 3): encrypt a file using system gpg for a
/// recipient. Returns `"1"` on success.
pub fn gpg_encrypt_file(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let in_path = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.encrypt.file expects (in_path, out_path, recipient)"))?;
    let out_path = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.encrypt.file expects (in_path, out_path, recipient)"))?;
    let recipient = args
        .get(2)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.encrypt.file expects (in_path, out_path, recipient)"))?;

    run_gpg_or_fail(
        &["--encrypt", "--recipient", recipient, "--output", out_path, in_path],
        None,
    )?;
    Ok(DyylValue::Str("1".to_string()))
}

/// `gpg.decrypt` (arity 1): decrypt an armored message using system gpg.
pub fn gpg_decrypt(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let armor = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.decrypt expects (armor)"))?;
    let stdout = run_gpg_or_fail(&["--decrypt"], Some(armor.as_bytes()))?;
    Ok(DyylValue::Str(stdout))
}

/// `gpg.decrypt.file` (arity 2): decrypt a file using system gpg.
/// Returns `"1"` on success.
pub fn gpg_decrypt_file(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let in_path = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.decrypt.file expects (in_path, out_path)"))?;
    let out_path = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.decrypt.file expects (in_path, out_path)"))?;
    run_gpg_or_fail(&["--decrypt", "--output", out_path, in_path], None)?;
    Ok(DyylValue::Str("1".to_string()))
}

/// `gpg.sign` (arity 2+): sign text using system gpg (inline or detached).
pub fn gpg_sign(_state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.sign expects (text, key_id, detach?)"))?;
    let key_id = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.sign expects (text, key_id)"))?;
    let detach = args
        .get(2)
        .and_then(DyylValue::as_str)
        .map(|s| s == "1")
        .unwrap_or(false);

    let mut gpg_args: Vec<&str> = vec!["--armor", "--local-user", key_id];
    gpg_args.push(if detach { "--detach-sign" } else { "--sign" });
    let stdout = run_gpg_or_fail(&gpg_args, Some(text.as_bytes()))?;
    Ok(DyylValue::Str(stdout))
}

/// `gpg.sign.file` (arity 3+): sign a file using system gpg (inline or
/// detached). Returns `"1"` on success.
pub fn gpg_sign_file(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let in_path = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.sign.file expects (in_path, out_path, key_id, detach?)"))?;
    let out_path = args
        .get(1)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.sign.file expects (in_path, out_path, key_id)"))?;
    let key_id = args
        .get(2)
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.sign.file expects (in_path, out_path, key_id)"))?;
    let detach = args
        .get(3)
        .and_then(DyylValue::as_str)
        .map(|s| s == "1")
        .unwrap_or(false);

    let mut gpg_args: Vec<&str> =
        vec!["--armor", "--local-user", key_id, "--output", out_path];
    gpg_args.push(if detach { "--detach-sign" } else { "--sign" });
    gpg_args.push(in_path);
    run_gpg_or_fail(&gpg_args, None)?;
    Ok(DyylValue::Str("1".to_string()))
}

/// Build the verify-result dict `{valid, signer}`.
fn make_verify_result(valid: bool, signer: &str) -> DyylValue {
    DyylValue::Dict(vec![
        pair("valid", if valid { "1" } else { "0" }),
        pair("signer", signer),
    ])
}

/// `gpg.verify` (arity 2): verify a signature using system gpg (inline
/// or detached). Returns `{valid, signer}` — never errors on a bad sig;
/// the `valid` field carries that signal.
pub fn gpg_verify(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let sig_or_text = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.verify expects (sig_or_text, data?)"))?;
    let data = args.get(1).and_then(DyylValue::as_str);

    if let Some(data_text) = data {
        // Detached verification: write sig and data to temp files, then
        // `gpg --verify <sig> <data>`.
        let sig_temp = tempfile::NamedTempFile::new()
            .map_err(|e| PluginError::runtime(format!("create temp: {e}")))?;
        std::fs::write(sig_temp.path(), sig_or_text.as_bytes())
            .map_err(|e| PluginError::runtime(format!("write temp: {e}")))?;
        let data_temp = tempfile::NamedTempFile::new()
            .map_err(|e| PluginError::runtime(format!("create temp: {e}")))?;
        std::fs::write(data_temp.path(), data_text.as_bytes())
            .map_err(|e| PluginError::runtime(format!("write temp: {e}")))?;

        let sig_path = sig_temp.path().to_string_lossy().to_string();
        let data_path = data_temp.path().to_string_lossy().to_string();
        let (_stdout, stderr, code) =
            run_gpg(&["--verify", &sig_path, &data_path], None)?;
        let valid = code == 0;
        let signer = extract_signer_from_gpg_output(&stderr);
        Ok(make_verify_result(valid, &signer))
    } else {
        // Inline verification: the input is a signed message (sig+data).
        let (_stdout, stderr, code) = run_gpg(&["--verify"], Some(sig_or_text.as_bytes()))?;
        let valid = code == 0;
        let signer = extract_signer_from_gpg_output(&stderr);
        Ok(make_verify_result(valid, &signer))
    }
}

/// `gpg.verify.file` (arity 2+): verify a signature file using system gpg.
pub fn gpg_verify_file(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let sig_path = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.verify.file expects (sig_path, data_path?)"))?;
    let data_path = args.get(1).and_then(DyylValue::as_str);

    let gpg_args: Vec<&str> = if let Some(dp) = data_path {
        vec!["--verify", sig_path, dp]
    } else {
        vec!["--verify", sig_path]
    };

    let (_stdout, stderr, code) = run_gpg(&gpg_args, None)?;
    let valid = code == 0;
    let signer = extract_signer_from_gpg_output(&stderr);
    Ok(make_verify_result(valid, &signer))
}

/// `gpg.key.list` (arity 0): list keys in system gpg keyring. Returns a
/// list of `{fp, uid}` dicts.
pub fn key_list(
    _state: &mut PluginState,
    _args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    // On any gpg failure, return an empty list rather than erroring.
    let stdout =
        run_gpg_or_fail(&["--list-keys", "--with-colons"], None).unwrap_or_default();

    let mut keys: Vec<DyylValue> = Vec::new();
    let mut current_fp = String::new();
    let mut current_uid = String::new();

    for line in stdout.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 10 {
            continue;
        }
        let kind = fields.first().copied().unwrap_or("");
        match kind {
            "pub" | "sec" => {
                if !current_fp.is_empty() {
                    keys.push(DyylValue::Dict(vec![
                        pair("fp", &current_fp),
                        pair("uid", &current_uid),
                    ]));
                }
                current_fp = fields.get(4).copied().unwrap_or("").to_string();
                current_uid = fields.get(9).copied().unwrap_or("").to_string();
            }
            "fpr" => {
                // The full fingerprint follows the pub/sec line.
                current_fp = fields.get(9).copied().unwrap_or("").to_string();
            }
            "uid" => {
                current_uid = fields.get(9).copied().unwrap_or("").to_string();
            }
            _ => {}
        }
    }
    if !current_fp.is_empty() {
        keys.push(DyylValue::Dict(vec![
            pair("fp", &current_fp),
            pair("uid", &current_uid),
        ]));
    }

    Ok(DyylValue::List(keys))
}

/// `gpg.key.import` (arity 1): import armored key into system gpg
/// keyring. Returns the import count as a string.
pub fn key_import(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let armor = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.key.import expects (armor)"))?;

    let (_stdout, stderr, code) = run_gpg(&["--import"], Some(armor.as_bytes()))?;
    if code != 0 {
        return Err(PluginError::gpg_exec_failed(format!(
            "gpg --import exit {code}: {stderr}"
        )));
    }
    // Parse `gpg: Total number processed: N` from stderr.
    const MARKER: &str = "Total number processed:";
    let count = stderr
        .lines()
        .find_map(|line| {
            line.split_once(MARKER)
                .map(|(_, n)| n.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "1".to_string());

    Ok(DyylValue::Str(count))
}

/// `gpg.key.export` (arity 2): export key from system gpg keyring as
/// armored text.
pub fn key_export(
    _state: &mut PluginState,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    let key_id = args
        .first()
        .and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("gpg.key.export expects (key_id, secret?)"))?;
    let secret = args
        .get(1)
        .and_then(DyylValue::as_str)
        .map(|s| s == "1")
        .unwrap_or(false);

    let mut gpg_args: Vec<&str> = vec!["--armor"];
    gpg_args.push(if secret {
        "--export-secret-keys"
    } else {
        "--export"
    });
    gpg_args.push(key_id);

    let stdout = run_gpg_or_fail(&gpg_args, None)?;
    Ok(DyylValue::Str(stdout))
}
