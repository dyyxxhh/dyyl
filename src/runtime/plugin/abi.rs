//! C ABI types and function signatures for the plugin protocol.
//!
//! Each plugin must export these 15 symbols (see spec §4.1):
//!   dyyl_plugin_get_api_version
//!   dyyl_plugin_get_name
//!   dyyl_plugin_get_version
//!   dyyl_plugin_get_author
//!   dyyl_plugin_get_description
//!   dyyl_plugin_init
//!   dyyl_plugin_on_load
//!   dyyl_plugin_list_commands
//!   dyyl_plugin_get_command_help
//!   dyyl_plugin_handle_command
//!   dyyl_plugin_on_error
//!   dyyl_plugin_on_unload
//!   dyyl_plugin_shutdown
//!   dyyl_plugin_free_string
//!   dyyl_plugin_set_credentials
//!
//! All strings are UTF-8, NUL-terminated, malloc'd by the plugin, freed by
//! the plugin via dyyl_plugin_free_string.

/// The dyyl plugin API version this dyyl build supports.
pub const DYRL_API_VERSION: u32 = 2;

/// Type alias for the plugin handle (opaque pointer returned by init).
pub type PluginHandle = *mut std::ffi::c_void;

/// Error from ABI operations.
#[derive(Debug)]
pub enum AbiError {
    /// A required symbol is missing from the library.
    SymbolMissing(String),
    /// init() returned NULL.
    InitFailed,
    /// on_load() returned non-zero.
    OnLoadFailed(i32),
    /// handle_command() returned non-zero; carries the return code.
    CommandFailed(i32),
    /// A string from the plugin was invalid UTF-8.
    InvalidUtf8,
    /// set_credentials 返回非 0。
    SetCredentialsFailed(i32),
}

impl std::fmt::Display for AbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SymbolMissing(s) => write!(f, "missing symbol: {s}"),
            Self::InitFailed => write!(f, "init() returned NULL"),
            Self::OnLoadFailed(c) => write!(f, "on_load() failed with code {c}"),
            Self::CommandFailed(c) => write!(f, "handle_command() returned {c}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 from plugin"),
            Self::SetCredentialsFailed(c) => write!(f, "set_credentials() failed with code {c}"),
        }
    }
}

impl std::error::Error for AbiError {}

/// Function pointer types for the ABI symbols.
#[allow(clippy::missing_docs_in_private_items)]
pub mod symbols {
    use super::PluginHandle;
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    pub type GetApiVersion = unsafe extern "C" fn() -> c_uint;
    pub type GetString = unsafe extern "C" fn(*mut *mut c_char) -> c_int;
    pub type Init = unsafe extern "C" fn(c_uint) -> PluginHandle;
    pub type OnLoad = unsafe extern "C" fn(PluginHandle) -> c_int;
    pub type ListCommands = unsafe extern "C" fn(PluginHandle, *mut *mut c_char) -> c_int;
    pub type GetCommandHelp =
        unsafe extern "C" fn(PluginHandle, *const c_char, *mut *mut c_char) -> c_int;
    pub type HandleCommand =
        unsafe extern "C" fn(PluginHandle, *const c_char, *const c_char, *mut *mut c_char) -> c_int;
    pub type OnError =
        unsafe extern "C" fn(PluginHandle, *const c_char, c_int, *const c_char) -> c_int;
    pub type OnUnload = unsafe extern "C" fn(PluginHandle) -> c_int;
    pub type Shutdown = unsafe extern "C" fn(PluginHandle);
    pub type FreeString = unsafe extern "C" fn(*mut c_char);
    pub type OnErrorRaw = unsafe extern "C" fn(PluginHandle, *const c_char, c_int, *const c_char);
    pub type SetCredentials = unsafe extern "C" fn(PluginHandle, *const c_char) -> c_int;
}

/// Names of the 15 required symbols, in order.
#[must_use]
pub fn required_symbol_names() -> [&'static str; 15] {
    [
        "dyyl_plugin_get_api_version",
        "dyyl_plugin_get_name",
        "dyyl_plugin_get_version",
        "dyyl_plugin_get_author",
        "dyyl_plugin_get_description",
        "dyyl_plugin_init",
        "dyyl_plugin_on_load",
        "dyyl_plugin_list_commands",
        "dyyl_plugin_get_command_help",
        "dyyl_plugin_handle_command",
        "dyyl_plugin_on_error",
        "dyyl_plugin_on_unload",
        "dyyl_plugin_shutdown",
        "dyyl_plugin_free_string",
        "dyyl_plugin_set_credentials",
    ]
}
