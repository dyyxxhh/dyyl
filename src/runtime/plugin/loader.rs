//! dlopen + symbol resolution + dispatch.
//!
//! Opens a plugin dynamic library with `libloading`, resolves the 15
//! required ABI symbols, and provides typed methods to call them.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::Path;

use libloading::Library;

use crate::runtime::plugin::abi::{symbols, AbiError, DYRL_API_VERSION};

/// Loaded plugin — holds the dlopen'd library and resolved symbols.
#[derive(Debug)]
pub struct PluginLoader {
    library: Library,
    handle: *mut c_void,
}

// The handle is an opaque pointer from the plugin. We send it between threads
// (PluginManager is behind a Mutex but dispatch may come from different threads
// in future). The plugin is responsible for thread-safety of its handle.
unsafe impl Send for PluginLoader {}
unsafe impl Sync for PluginLoader {}

impl PluginLoader {
    /// Open a plugin library, verify API version, call `init`, call `on_load`.
    ///
    /// Returns the loaded plugin or an `AbiError`.
    pub fn load(
        path: &Path,
        plugin_name: &str,
        credentials_json: Option<&str>,
    ) -> Result<Self, AbiError> {
        unsafe {
            let library = Library::new(path).map_err(|e| {
                AbiError::SymbolMissing(format!("dlopen failed for {plugin_name}: {e}"))
            })?;

            // 1. get_api_version
            let get_api_version: symbols::GetApiVersion = *library
                .get(b"dyyl_plugin_get_api_version\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_get_api_version".to_string()))?;
            let plugin_api_version = get_api_version();
            // 支持 v1 和 v2 插件。v2 才有 set_credentials。
            if plugin_api_version != 1 && plugin_api_version != DYRL_API_VERSION {
                std::mem::drop(library);
                return Err(AbiError::SymbolMissing(format!(
                    "API version mismatch: plugin={plugin_api_version}, dyyl supports 1 and {DYRL_API_VERSION}"
                )));
            }

            // 2. init
            let init: symbols::Init = *library
                .get(b"dyyl_plugin_init\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_init".to_string()))?;
            let handle = init(DYRL_API_VERSION);
            if handle.is_null() {
                std::mem::drop(library);
                return Err(AbiError::InitFailed);
            }

            // 3. set_credentials（仅当传入了 credentials_json 时）。
            if let Some(json) = credentials_json {
                let set_creds: symbols::SetCredentials = *library
                    .get(b"dyyl_plugin_set_credentials\0")
                    .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_set_credentials".to_string()))?;
                let json_c = CString::new(json).map_err(|_| AbiError::InvalidUtf8)?;
                let rc = set_creds(handle, json_c.as_ptr());
                if rc != 0 {
                    std::mem::drop(library);
                    return Err(AbiError::SetCredentialsFailed(rc));
                }
            }

            let mut loader = Self { library, handle };

            // 4. on_load
            let on_load: symbols::OnLoad = *loader
                .library
                .get(b"dyyl_plugin_on_load\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_on_load".to_string()))?;
            let rc = on_load(loader.handle);
            if rc != 0 {
                loader.shutdown_internal();
                return Err(AbiError::OnLoadFailed(rc));
            }

            Ok(loader)
        }
    }

    /// Call `list_commands` and return the JSON string.
    pub fn list_commands(&self) -> Result<String, AbiError> {
        unsafe {
            let list_commands: symbols::ListCommands = *self
                .library
                .get(b"dyyl_plugin_list_commands\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_list_commands".to_string()))?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = list_commands(self.handle, &raw mut out_ptr);
            if rc != 0 || out_ptr.is_null() {
                return Err(AbiError::CommandFailed(rc));
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr
                .to_str()
                .map_err(|_| AbiError::InvalidUtf8)?
                .to_string();
            self.free_string(out_ptr);
            Ok(s)
        }
    }

    /// Call `handle_command`. `cmd_name` may contain dots (e.g. "user.login").
    /// `args_json` is the JSON-encoded args array. Returns the JSON-encoded
    /// result value.
    pub fn handle_command(&self, cmd_name: &str, args_json: &str) -> Result<String, AbiError> {
        unsafe {
            let handle_command: symbols::HandleCommand = *self
                .library
                .get(b"dyyl_plugin_handle_command\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_handle_command".to_string()))?;
            let cmd_c = CString::new(cmd_name).map_err(|_| AbiError::InvalidUtf8)?;
            let args_c = CString::new(args_json).map_err(|_| AbiError::InvalidUtf8)?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = handle_command(
                self.handle,
                cmd_c.as_ptr(),
                args_c.as_ptr(),
                &raw mut out_ptr,
            );
            if rc != 0 {
                // out_ptr may still hold an error object — free it if present.
                if !out_ptr.is_null() {
                    self.free_string(out_ptr);
                }
                return Err(AbiError::CommandFailed(rc));
            }
            if out_ptr.is_null() {
                return Ok(String::from("null"));
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr
                .to_str()
                .map_err(|_| AbiError::InvalidUtf8)?
                .to_string();
            self.free_string(out_ptr);
            Ok(s)
        }
    }

    /// Call `get_command_help` for a specific command.
    pub fn get_command_help(&self, cmd_name: &str) -> Result<String, AbiError> {
        unsafe {
            let get_help: symbols::GetCommandHelp = *self
                .library
                .get(b"dyyl_plugin_get_command_help\0")
                .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_get_command_help".to_string()))?;
            let cmd_c = CString::new(cmd_name).map_err(|_| AbiError::InvalidUtf8)?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = get_help(self.handle, cmd_c.as_ptr(), &raw mut out_ptr);
            if rc != 0 || out_ptr.is_null() {
                return Ok(String::new());
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr
                .to_str()
                .map_err(|_| AbiError::InvalidUtf8)?
                .to_string();
            self.free_string(out_ptr);
            Ok(s)
        }
    }

    /// Free a string allocated by the plugin.
    fn free_string(&self, ptr: *mut c_char) {
        unsafe {
            let free_string: symbols::FreeString =
                match self.library.get(b"dyyl_plugin_free_string\0") {
                    Ok(f) => *f,
                    Err(_) => return, // Can't free — leak rather than crash.
                };
            free_string(ptr);
        }
    }

    /// Call `on_unload` then `shutdown`.
    fn shutdown_internal(&mut self) {
        unsafe {
            if let Ok(on_unload) = self
                .library
                .get::<symbols::OnUnload>(b"dyyl_plugin_on_unload\0")
            {
                let on_unload = *on_unload;
                let _ = on_unload(self.handle);
            }
            if let Ok(shutdown) = self
                .library
                .get::<symbols::Shutdown>(b"dyyl_plugin_shutdown\0")
            {
                let shutdown = *shutdown;
                shutdown(self.handle);
            }
        }
    }
}

impl Drop for PluginLoader {
    fn drop(&mut self) {
        self.shutdown_internal();
        // Library drops here, closing the dlopen handle.
    }
}
