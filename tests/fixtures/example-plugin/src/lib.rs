//! Example plugin for dyyl — implements greet and math.double commands.
//!
//! Compiled as cdylib, loaded by dyyl's plugin system for integration tests.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

static mut HANDLE: *mut c_void = ptr::null_mut();

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_api_version() -> c_uint {
    1
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_name(out: *mut *mut c_char) -> c_int {
    write_string("example", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_version(out: *mut *mut c_char) -> c_int {
    write_string("0.1.0", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_author(out: *mut *mut c_char) -> c_int {
    write_string("dyyl-test", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_description(out: *mut *mut c_char) -> c_int {
    write_string("Example plugin for integration tests", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_init(_api_version: c_uint) -> *mut c_void {
    // Use a static sentinel as the "handle".
    unsafe {
        HANDLE = 1 as *mut c_void;
        HANDLE
    }
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_load(_handle: *mut c_void) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_list_commands(
    _handle: *mut c_void,
    out: *mut *mut c_char,
) -> c_int {
    let json = r#"[{"name":"greet","arity":1,"brief":"Send a greeting"},{"name":"math.double","arity":1,"brief":"Double a number"}]"#;
    write_string(json, out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_command_help(
    _handle: *mut c_void,
    _cmd: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    write_string("Help text", out)
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_handle_command(
    _handle: *mut c_void,
    cmd: *const c_char,
    args: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    let cmd_str = unsafe { CStr::from_ptr(cmd) }.to_str().unwrap_or("");
    let args_str = unsafe { CStr::from_ptr(args) }.to_str().unwrap_or("[]");

    match cmd_str {
        "greet" => {
            // args is [{"type":"str","value":"..."}] — extract first value.
            let name = extract_first_str_arg(args_str);
            let result = format!(r#"{{"type":"str","value":"Hello, {name}!"}}"#);
            write_string(&result, out)
        }
        "math.double" => {
            let n = extract_first_num_arg(args_str);
            let doubled = n * 2;
            let result = format!(r#"{{"type":"num","value":"{doubled}"}}"#);
            write_string(&result, out)
        }
        _ => {
            let err = r#"{"code":"unknown_command","message":"unknown command"}"#;
            write_string(err, out);
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_error(
    _handle: *mut c_void,
    _cmd: *const c_char,
    _code: c_int,
    _err: *const c_char,
) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_on_unload(_handle: *mut c_void) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_shutdown(_handle: *mut c_void) {
    unsafe {
        HANDLE = ptr::null_mut();
    }
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_free_string(ptr: *mut c_char) {
    unsafe {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn write_string(s: &str, out: *mut *mut c_char) -> c_int {
    let c = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    unsafe {
        *out = c.into_raw();
    }
    0
}

fn extract_first_str_arg(args_json: &str) -> String {
    // Naive parse: find "value":"..." in args_json.
    if let Some(pos) = args_json.find("\"value\":\"") {
        let rest = &args_json[pos + 9..];
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    "world".to_string()
}

fn extract_first_num_arg(args_json: &str) -> i64 {
    if let Some(pos) = args_json.find("\"value\":\"") {
        let rest = &args_json[pos + 9..];
        if let Some(end) = rest.find('"') {
            return rest[..end].parse().unwrap_or(0);
        }
    }
    0
}
