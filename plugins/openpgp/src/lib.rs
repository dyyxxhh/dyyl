//! OpenPGP plugin for dyyl — exposes 30 commands (17 sequoia-based +
//! 13 gpg-wrapper) through dyyl's C ABI (15 symbols, ABI v2).
//!
//! Built as a `cdylib` (`libopenpgp.so` / `.dll` / `.dylib`), loaded by
//! dyyl's plugin system via dlopen. State is owned by the handle:
//! `init` allocates a `Box<PluginState>`, `shutdown` frees it.

// FFI entry points dereference raw pointers passed from the host. Marking
// them `unsafe` is not meaningful (the C caller has no `unsafe` concept),
// and the host already wraps calls in `unsafe` via dlsym'd function
// pointers. See `src/runtime/plugin/abi.rs` for the caller-side types.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Scaffold stage: many error constructors, value accessors, and command
// stubs are intentionally unused until Task 6+ wires them up.
#![allow(dead_code)]

mod codec;
mod commands;
mod creds;
mod error;
pub mod keyring;
pub mod state;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};

use state::PluginState;

// ── 1. API version ────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_api_version() -> c_uint {
    2
}

// ── 2–5. Metadata getters ─────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_name(out: *mut *mut c_char) -> c_int {
    write_string("openpgp", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_version(out: *mut *mut c_char) -> c_int {
    write_string("0.1.0", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_author(out: *mut *mut c_char) -> c_int {
    write_string("dyyl", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_description(out: *mut *mut c_char) -> c_int {
    write_string("OpenPGP plugin using sequoia-openpgp", out)
}

// ── 6. init ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_init(_api_version: c_uint) -> *mut c_void {
    let state = Box::new(PluginState::default());
    Box::into_raw(state) as *mut c_void
}

// ── 7. on_load ────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_load(_handle: *mut c_void) -> c_int {
    0
}

// ── 8. list_commands ──────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_list_commands(
    _handle: *mut c_void,
    out: *mut *mut c_char,
) -> c_int {
    write_string(include_str!("../command_list.json"), out)
}

// ── 9. get_command_help ───────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_command_help(
    _handle: *mut c_void,
    _cmd: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    write_string(
        "OpenPGP plugin command. See command_list.json for the full command table.",
        out,
    )
}

// ── 10. handle_command ────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_handle_command(
    handle: *mut c_void,
    cmd: *const c_char,
    args: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    if handle.is_null() {
        error::write_error(out, "runtime", "null plugin handle");
        return 1;
    }

    let cmd_str = if cmd.is_null() {
        ""
    } else {
        // SAFETY: `cmd` is a valid NUL-terminated C string per the ABI contract.
        unsafe { CStr::from_ptr(cmd) }.to_str().unwrap_or("")
    };
    let args_str = if args.is_null() {
        "[]"
    } else {
        // SAFETY: `args` is a valid NUL-terminated C string per the ABI contract.
        unsafe { CStr::from_ptr(args) }.to_str().unwrap_or("[]")
    };

    // SAFETY: `handle` was produced by `dyyl_plugin_init` via
    // `Box::into_raw` and is still valid (not yet freed by `shutdown`).
    let state: &mut PluginState = unsafe { &mut *(handle as *mut PluginState) };

    match codec::decode_args(args_str) {
        Ok(args_vec) => match commands::dispatch(state, cmd_str, &args_vec) {
            Ok(v) => {
                codec::encode_out(out, &v);
                0
            }
            Err(e) => {
                error::write_error(out, e.code(), e.message());
                1
            }
        },
        Err(msg) => {
            error::write_error(out, "parse_failed", &msg);
            1
        }
    }
}

// ── 11. on_error ──────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_error(
    _handle: *mut c_void,
    _cmd: *const c_char,
    _code: c_int,
    _err: *const c_char,
) -> c_int {
    0
}

// ── 12. on_unload ─────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_unload(handle: *mut c_void) -> c_int {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: `handle` was produced by `dyyl_plugin_init` via
    // `Box::into_raw` and is still valid.
    let state: &mut PluginState = unsafe { &mut *(handle as *mut PluginState) };
    state.clear_cache();
    0
}

// ── 13. shutdown ──────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_shutdown(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    // SAFETY: `handle` was produced by `dyyl_plugin_init` via
    // `Box::into_raw`; this consumes and drops it exactly once.
    unsafe {
        let _ = Box::from_raw(handle as *mut PluginState);
    }
}

// ── 14. free_string ───────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr` was produced by `CString::into_raw`; this consumes
    // and frees it exactly once.
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

// ── 15. set_credentials ───────────────────────────────────────────

#[no_mangle]
pub extern "C" fn dyyl_plugin_set_credentials(
    handle: *mut c_void,
    creds_json: *const c_char,
) -> c_int {
    if handle.is_null() || creds_json.is_null() {
        return 1;
    }
    // SAFETY: `handle` was produced by `dyyl_plugin_init` via
    // `Box::into_raw` and is still valid.
    let state: &mut PluginState = unsafe { &mut *(handle as *mut PluginState) };
    // SAFETY: `creds_json` is a valid NUL-terminated C string per the ABI contract.
    let json_str = unsafe { CStr::from_ptr(creds_json) }
        .to_str()
        .unwrap_or("");
    match creds::apply_credentials(state, json_str) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

// ── Helpers ───────────────────────────────────────────────────────

/// Write `s` to the `out` parameter as a `CString` (allocates via
/// `CString::into_raw`). Returns 0 on success.
///
/// Contract: `out` must be a valid, non-null pointer to a `*mut c_char`
/// slot owned by the caller.
fn write_string(s: &str, out: *mut *mut c_char) -> c_int {
    let c = cstring_from_str(s);
    // SAFETY: caller guarantees `out` is a valid pointer to a slot.
    unsafe {
        *out = c.into_raw();
    }
    0
}

/// Build a `CString` from `&str`, stripping any NUL bytes (which would
/// otherwise make `CString::new` fail). After stripping, construction is
/// infallible; the `unwrap_or_else` branch is unreachable but kept to
/// satisfy the type system without `unwrap`/`expect` (denied by clippy).
fn cstring_from_str(s: &str) -> CString {
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != 0).collect();
    CString::new(bytes).unwrap_or_else(|_| empty_cstring())
}

/// Returns a guaranteed-valid empty `CString`.
fn empty_cstring() -> CString {
    // SAFETY: A single NUL byte is a valid CString representing the empty
    // string (no interior NULs, last byte is NUL).
    unsafe { CString::from_vec_with_nul_unchecked(vec![0u8]) }
}
