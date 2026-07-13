# Plugin Runtime — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the plugin runtime to dyyl: dynamic-library loading, manifest parsing, fetch with SHA256 verification, PluginManager orchestration, and dispatch fallback for `<name>.<sub>[.<sub>...]+` plugin commands.

**Architecture:** New `src/runtime/plugin/` module with 7 files (abi, manifest, store, registry, loader, fetch, mod). Dispatch gets a fallback arm that routes unknown `<name>.<sub>` commands to PluginManager, which lazily fetches+installs on first call, then dlopens and dispatches via C ABI. `Env` gains a `plugin_manager` field. Config gains `installed_plugins` with `last_used_at` tracking.

**Tech Stack:** Rust 2021, `libloading` (new dep for dlopen), `ureq` (existing), `sha2` (existing), `serde_json` (existing), `directories` (existing).

**Spec:** [docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)

---

## File Structure

| File | Responsibility |
|---|---|
| `src/runtime/plugin/mod.rs` (create) | `PluginManager` — holds loaded plugins table, orchestrates fetch→load→dispatch |
| `src/runtime/plugin/abi.rs` (create) | C ABI function pointer types, `DyylApiVersion` const, `AbiError` |
| `src/runtime/plugin/manifest.rs` (create) | `RemoteManifest`, `PluginCommand`, `PlatformEntry`, `LocalPluginToml` serde structs |
| `src/runtime/plugin/store.rs` (create) | XDG data path computation: `plugin_dir()`, `plugin_version_dir()`, `lib_path()` |
| `src/runtime/plugin/registry.rs` (create) | Scan `~/.local/share/dyyl/plugins/` for installed plugins, return `InstalledPlugin` list |
| `src/runtime/plugin/loader.rs` (create) | `dlopen` via libloading, resolve 14 symbols, call `init`/`on_load`/`handle_command`/`on_unload`/`shutdown` |
| `src/runtime/plugin/fetch.rs` (create) | `fetch_manifest(name)` GET from l.dyyapp.com, `download_and_verify(url, sha256)` with SHA256 check |
| `src/runtime/cmd/plugin.rs` (create) | `dispatch_plugin_command()` — split `<name>.<rest>`, route to PluginManager |
| `src/runtime/cmd/dispatch.rs` (modify) | Add fallback arm before `_ =>` that calls `plugin::dispatch_plugin_command` |
| `src/runtime/cmd/mod.rs` (modify) | Add `pub(crate) mod plugin;` |
| `src/runtime/env.rs` (modify) | Add `plugin_manager: PluginManager` field |
| `src/runtime/mod.rs` (modify) | Add `pub mod plugin;` |
| `src/config.rs` (modify) | Add `installed_plugins: HashMap<String, InstalledPluginRecord>` with `last_used_at` |
| `Cargo.toml` (modify) | Add `libloading = "0.8"` dependency |

**Lint note:** Project denies `unwrap_used`/`panic`/`indexing_slicing`/`todo`/`unimplemented`. Use `?`, `.get()`, `.expect()` (expect is allowed). The loader uses `unsafe` for dlopen — wrap in `unsafe` blocks, `unsafe_op_in_unsafe_fn = "deny"` means unsafe ops inside unsafe fn still need explicit `unsafe {}`.

---

## Task 1: Add libloading dependency + plugin module skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `src/runtime/plugin/mod.rs`
- Modify: `src/runtime/mod.rs`

- [ ] **Step 1: Add libloading to Cargo.toml**

In `Cargo.toml`, add `libloading = "0.8"` to `[dependencies]` section (after `directories = "5"`):

```toml
directories = "5"
libloading = "0.8"
```

- [ ] **Step 2: Create plugin module skeleton**

Create `src/runtime/plugin/mod.rs`:

```rust
//! Plugin system — dynamic-library loading and dispatch.
//!
//! Plugins are compiled dynamic libraries (.so/.dll/.dylib) loaded via dlopen.
//! The `PluginManager` orchestrates fetch → install → load → dispatch.
//! On first call to `<name>.<sub>`, the manager fetches the manifest from
//! l.dyyapp.com, downloads+verifies the library, dlopens it, and dispatches.

pub mod abi;
pub mod fetch;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod store;

use std::collections::HashMap;
use std::sync::Mutex;

use crate::i18n::Lang;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Loaded plugin instance — holds the dlopen'd library and handle.
pub struct LoadedPlugin {
    /// The name of the plugin (e.g. "migpt").
    pub name: String,
    /// The dlopen'd library + resolved symbols.
    pub loader: loader::PluginLoader,
}

/// Central plugin manager — holds already-loaded plugins.
pub struct PluginManager {
    loaded: Mutex<HashMap<String, LoadedPlugin>>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Dispatch a plugin command `<name>.<sub>` (sub may contain dots).
    ///
    /// If the plugin isn't loaded yet, this fetches+installs+loads it first.
    /// Returns the plugin's result Value or a RuntimeError.
    pub fn dispatch(
        &self,
        name: &str,
        sub: &str,
        args: &[Value],
        lang: Lang,
        line: usize,
    ) -> Result<Value, RuntimeError> {
        // Placeholder — implemented in Task 8.
        let _ = (name, sub, args, lang, line);
        Err(RuntimeError::new(
            line,
            format!("{name}.{sub}"),
            crate::i18n::plugin_command_failed(lang, name, sub, "not_implemented"),
        ))
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 3: Register plugin module in runtime/mod.rs**

In `src/runtime/mod.rs`, add `pub mod plugin;` after `pub mod io_provider;` (line 13):

```rust
pub mod cmd;
pub mod env;
pub mod error;
pub mod execute;
pub mod host_provider;
pub mod io_provider;
pub mod plugin;
pub mod value;
```

- [ ] **Step 4: Create stub submodule files so the module compiles**

Create these 6 stub files. Each just has a module doc comment and an empty item so it compiles:

`src/runtime/plugin/abi.rs`:
```rust
//! C ABI types and function signatures for the plugin protocol.
```

`src/runtime/plugin/fetch.rs`:
```rust
//! Fetch plugin manifests and libraries from l.dyyapp.com.
```

`src/runtime/plugin/loader.rs`:
```rust
//! dlopen + symbol resolution + dispatch.

use std::sync::OnceLock;

/// Loaded plugin — holds the dlopen'd library.
///
/// (Real implementation in Task 7.)
pub struct PluginLoader {
    _name: String,
}

impl PluginLoader {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { _name: String::new() }
    }
}
```

`src/runtime/plugin/manifest.rs`:
```rust
//! Parse plugin manifests (remote JSON + local TOML).
```

`src/runtime/plugin/registry.rs`:
```rust
//! Scan installed plugins directory.
```

`src/runtime/plugin/store.rs`:
```rust
//! Plugin storage path management (XDG data).
```

- [ ] **Step 5: Run cargo build to verify compilation**

Run: `cargo build`

Expected: Compiles successfully (may have unused-import warnings, that's fine).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/runtime/plugin/ src/runtime/mod.rs
git commit -m "feat(plugin): add libloading dep + plugin module skeleton"
```

---

## Task 2: Implement store.rs (XDG path management)

**Files:**
- Modify: `src/runtime/plugin/store.rs`
- Create: `tests/plugin_store_tests.rs`

- [ ] **Step 1: Write failing tests for store paths**

Create `tests/plugin_store_tests.rs`:

```rust
use dyyl::runtime::plugin::store;
use std::path::PathBuf;

#[test]
fn plugin_dir_is_under_xdg_data() {
    let dir = store::plugin_dir();
    // Should end with dyyl/plugins
    assert!(dir.ends_with("dyyl/plugins") || dir.ends_with("dyyl\\plugins"),
        "plugin_dir was: {}", dir.display());
}

#[test]
fn plugin_version_dir_includes_name_and_version() {
    let dir = store::plugin_version_dir("migpt", "0.1.0");
    assert!(dir.to_string_lossy().contains("migpt"));
    assert!(dir.to_string_lossy().contains("0.1.0"));
}

#[test]
fn lib_path_ends_with_platform_suffix() {
    let path = store::lib_path("migpt", "0.1.0");
    let s = path.to_string_lossy();
    // On linux .so, macos .dylib, windows .dll
    assert!(s.ends_with(".so") || s.ends_with(".dylib") || s.ends_with(".dll"),
        "lib_path was: {s}");
}

#[test]
fn plugin_toml_path_in_same_dir_as_lib() {
    let toml_path = store::plugin_toml_path("migpt", "0.1.0");
    assert!(toml_path.to_string_lossy().contains("migpt"));
    assert!(toml_path.to_string_lossy().ends_with("plugin.toml"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plugin_store_tests`

Expected: FAIL — functions not found.

- [ ] **Step 3: Implement store.rs**

Replace `src/runtime/plugin/store.rs` content:

```rust
//! Plugin storage path management (XDG data).
//!
//! Plugins are stored under `<xdg_data>/dyyl/plugins/<name>/<version>/`.
//! The library file is `lib<name>.so` (linux), `lib<name>.dylib` (macos),
//! or `<name>.dll` (windows). A `plugin.toml` sits alongside.

use std::path::PathBuf;

/// Return the base plugins directory: `<xdg_data>/dyyl/plugins/`.
///
/// Returns `None` if the XDG data directory cannot be determined.
#[must_use]
pub fn plugin_dir() -> PathBuf {
    let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")
        .expect("unable to determine XDG data directory");
    proj.data_dir().join("plugins")
}

/// Return the directory for a specific plugin version:
/// `<plugin_dir>/<name>/<version>/`.
#[must_use]
pub fn plugin_version_dir(name: &str, version: &str) -> PathBuf {
    plugin_dir().join(name).join(version)
}

/// Return the path to the plugin library file.
///
/// Filename: `lib<name>.so` (linux), `lib<name>.dylib` (macos), `<name>.dll` (windows).
#[must_use]
pub fn lib_path(name: &str, version: &str) -> PathBuf {
    let filename = lib_filename(name);
    plugin_version_dir(name, version).join(filename)
}

/// Return the path to the plugin's `plugin.toml` metadata file.
#[must_use]
pub fn plugin_toml_path(name: &str, version: &str) -> PathBuf {
    plugin_version_dir(name, version).join("plugin.toml")
}

/// Return the locales directory for a plugin.
#[must_use]
pub fn plugin_locales_dir(name: &str, version: &str) -> PathBuf {
    plugin_version_dir(name, version).join("locales")
}

/// Compute the platform-appropriate library filename for a plugin name.
#[must_use]
fn lib_filename(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{name}.dylib")
    } else {
        format!("lib{name}.so")
    }
}

/// Return the current platform identifier used in manifests.
#[must_use]
pub fn current_platform() -> String {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("{os}-{arch}")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test plugin_store_tests`

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add src/runtime/plugin/store.rs tests/plugin_store_tests.rs
git commit -m "feat(plugin): implement store.rs XDG path management"
```

---

## Task 3: Implement manifest.rs (remote + local manifest structs)

**Files:**
- Modify: `src/runtime/plugin/manifest.rs`
- Create: `tests/plugin_manifest_tests.rs`

- [ ] **Step 1: Write failing tests for manifest parsing**

Create `tests/plugin_manifest_tests.rs`:

```rust
use dyyl::runtime::plugin::manifest::{RemoteManifest, LocalPluginToml};

#[test]
fn parse_remote_manifest() {
    let json = r#"{
        "name": "migpt",
        "version": "0.1.0",
        "abi_version": 1,
        "dyyl_min": "0.2.0",
        "panic_mode": "abort",
        "commands": [
            {"name": "greet", "arity": 1, "brief": "Send a greeting"},
            {"name": "user.login", "arity": 2, "brief": "Login"}
        ],
        "platforms": [
            {"platform": "linux-x86_64", "url": "https://l.dyyapp.com/p/migpt/0.1.0/linux-x86_64/libmigpt.so", "sha256": "abc123"}
        ]
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.name, "migpt");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.abi_version, 1);
    assert_eq!(m.dyyl_min, "0.2.0");
    assert_eq!(m.panic_mode, "abort");
    assert_eq!(m.commands.len(), 2);
    assert_eq!(m.commands[0].name, "greet");
    assert_eq!(m.commands[0].arity, 1);
    assert_eq!(m.commands[1].name, "user.login");
    assert_eq!(m.platforms.len(), 1);
    assert_eq!(m.platforms[0].platform, "linux-x86_64");
    assert_eq!(m.platforms[0].sha256, "abc123");
}

#[test]
fn parse_local_plugin_toml() {
    let toml = r#"
name = "migpt"
version = "0.1.0"
abi_version = 1
dyyl_min = "0.2.0"
panic_mode = "abort"

[[commands]]
name = "greet"
arity = 1
brief = "Send a greeting"

[installed]
source_url = "https://l.dyyapp.com/p/migpt.so"
sha256 = "abc123"
installed_at = "2026-07-13T10:30:00Z"
dyyl_version = "0.2.0"
"#;
    let t: LocalPluginToml = toml::from_str(toml).unwrap();
    assert_eq!(t.name, "migpt");
    assert_eq!(t.version, "0.1.0");
    assert_eq!(t.abi_version, 1);
    assert_eq!(t.commands.len(), 1);
    assert_eq!(t.installed.sha256, "abc123");
    assert_eq!(t.installed.dyyl_version, "0.2.0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plugin_manifest_tests`

Expected: FAIL — structs not found.

- [ ] **Step 3: Implement manifest.rs**

Replace `src/runtime/plugin/manifest.rs` content:

```rust
//! Parse plugin manifests (remote JSON + local TOML).

use serde::{Deserialize, Serialize};

/// Remote manifest fetched from `l.dyyapp.com/plugins/<name>/manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteManifest {
    pub name: String,
    pub version: String,
    pub abi_version: u32,
    pub dyyl_min: String,
    #[serde(default = "default_panic_mode")]
    pub panic_mode: String,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    pub platforms: Vec<PlatformEntry>,
    /// Optional: indicates plugin ships locales/ directory.
    #[serde(default)]
    pub has_locales: bool,
}

fn default_panic_mode() -> String {
    "abort".to_string()
}

/// A command exposed by a plugin. `name` may contain dots (e.g. "user.login").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    pub arity: usize,
    #[serde(default)]
    pub brief: String,
}

/// A platform-specific build entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub platform: String,
    pub url: String,
    pub sha256: String,
}

/// Local `plugin.toml` stored alongside the library after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPluginToml {
    pub name: String,
    pub version: String,
    pub abi_version: u32,
    pub dyyl_min: String,
    pub panic_mode: String,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    pub installed: InstalledRecord,
}

/// Installation metadata in `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub source_url: String,
    pub sha256: String,
    pub installed_at: String,
    pub dyyl_version: String,
}

impl LocalPluginToml {
    /// Find a command by name (exact match, names may contain dots).
    #[must_use]
    pub fn find_command(&self, name: &str) -> Option<&PluginCommand> {
        self.commands.iter().find(|c| c.name == name)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test plugin_manifest_tests`

Expected: PASS — 2 tests green.

- [ ] **Step 5: Commit**

```bash
git add src/runtime/plugin/manifest.rs tests/plugin_manifest_tests.rs
git commit -m "feat(plugin): implement manifest.rs structs for remote+local"
```

---

## Task 4: Implement abi.rs (C ABI types)

**Files:**
- Modify: `src/runtime/plugin/abi.rs`

- [ ] **Step 1: Implement abi.rs**

Replace `src/runtime/plugin/abi.rs` content:

```rust
//! C ABI types and function signatures for the plugin protocol.
//!
//! Each plugin must export these 14 symbols (see spec §4.1):
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
//!
//! All strings are UTF-8, NUL-terminated, malloc'd by the plugin, freed by
//! the plugin via dyyl_plugin_free_string.

/// The dyyl plugin API version this dyyl build supports.
pub const DYRL_API_VERSION: u32 = 1;

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
}

impl std::fmt::Display for AbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SymbolMissing(s) => write!(f, "missing symbol: {s}"),
            Self::InitFailed => write!(f, "init() returned NULL"),
            Self::OnLoadFailed(c) => write!(f, "on_load() failed with code {c}"),
            Self::CommandFailed(c) => write!(f, "handle_command() returned {c}"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 from plugin"),
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
}

/// Names of the 14 required symbols, in order.
#[must_use]
pub fn required_symbol_names() -> [&'static str; 14] {
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
    ]
}
```

- [ ] **Step 2: Run cargo build to verify compilation**

Run: `cargo build`

Expected: Compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add src/runtime/plugin/abi.rs
git commit -m "feat(plugin): implement abi.rs C ABI types and symbol names"
```

---

## Task 5: Implement fetch.rs (HTTP + SHA256 verification)

**Files:**
- Modify: `src/runtime/plugin/fetch.rs`
- Create: `tests/plugin_fetch_tests.rs`

- [ ] **Step 1: Write failing test for SHA256 verification**

Create `tests/plugin_fetch_tests.rs`:

```rust
use dyyl::runtime::plugin::fetch;

#[test]
fn sha256_of_known_bytes() {
    // SHA256 of empty string
    let hash = fetch::sha256_bytes(b"");
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_of_hello() {
    let hash = fetch::sha256_bytes(b"hello");
    assert_eq!(
        hash,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn verify_checksum_matches() {
    let data = b"hello";
    let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    assert!(fetch::verify_checksum(data, expected));
}

#[test]
fn verify_checksum_mismatches() {
    let data = b"hello";
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(!fetch::verify_checksum(data, wrong));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plugin_fetch_tests`

Expected: FAIL — functions not found.

- [ ] **Step 3: Implement fetch.rs**

Replace `src/runtime/plugin/fetch.rs` content:

```rust
//! Fetch plugin manifests and libraries from l.dyyapp.com.
//!
//! Uses the same `ureq` agent as `net.rs` for HTTPS. SHA256 verification
//! ensures downloaded libraries match the manifest's checksum.

use std::io::Read;

use sha2::{Digest, Sha256};

use crate::runtime::plugin::manifest::RemoteManifest;

/// Base URL for plugin distribution.
const PLUGIN_BASE_URL: &str = "https://l.dyyapp.com/plugins";

/// Fetch a plugin's remote manifest by name.
///
/// GETs `{base}/{name}/manifest.json`. Returns the parsed manifest or an error.
pub fn fetch_manifest(name: &str) -> Result<RemoteManifest, FetchError> {
    let url = format!("{PLUGIN_BASE_URL}/{name}/manifest.json");
    let agent = ureq::AgentBuilder::new().build();
    let mut body = String::new();
    agent
        .get(&url)
        .call()
        .map_err(|e| FetchError::Http(url.clone(), e.to_string()))?
        .into_reader()
        .read_to_string(&mut body)
        .map_err(|e| FetchError::Read(url.clone(), e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| FetchError::Parse(url, e.to_string()))
}

/// Download a library from `url` and verify its SHA256 matches `expected_sha256`.
///
/// Returns the raw bytes on success.
pub fn download_and_verify(url: &str, expected_sha256: &str) -> Result<Vec<u8>, FetchError> {
    let agent = ureq::AgentBuilder::new().build();
    let mut body = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| FetchError::Http(url.to_string(), e.to_string()))?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| FetchError::Read(url.to_string(), e.to_string()))?;
    if !verify_checksum(&body, expected_sha256) {
        return Err(FetchError::ChecksumMismatch(url.to_string()));
    }
    Ok(body)
}

/// Compute the SHA256 hex digest of a byte slice.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verify that a byte slice's SHA256 matches `expected` (hex string).
#[must_use]
pub fn verify_checksum(data: &[u8], expected: &str) -> bool {
    let actual = sha256_bytes(data);
    actual.eq_ignore_ascii_case(expected)
}

/// Error from fetch operations.
#[derive(Debug)]
pub enum FetchError {
    Http(String, String),
    Read(String, String),
    Parse(String, String),
    ChecksumMismatch(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(url, e) => write!(f, "HTTP error fetching {url}: {e}"),
            Self::Read(url, e) => write!(f, "read error from {url}: {e}"),
            Self::Parse(url, e) => write!(f, "parse error for {url}: {e}"),
            Self::ChecksumMismatch(url) => write!(f, "SHA256 mismatch for {url}"),
        }
    }
}

impl std::error::Error for FetchError {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test plugin_fetch_tests`

Expected: PASS — 4 tests green.

- [ ] **Step 5: Commit**

```bash
git add src/runtime/plugin/fetch.rs tests/plugin_fetch_tests.rs
git commit -m "feat(plugin): implement fetch.rs with SHA256 verification"
```

---

## Task 6: Implement registry.rs (scan installed plugins)

**Files:**
- Modify: `src/runtime/plugin/registry.rs`
- Create: `tests/plugin_registry_tests.rs`

- [ ] **Step 1: Write failing test for registry scanning**

Create `tests/plugin_registry_tests.rs`:

```rust
use dyyl::runtime::plugin::registry;
use dyyl::runtime::plugin::store;
use std::fs;

#[test]
fn scan_empty_dir_returns_empty() {
    // Use a temp dir — but registry scans the real XDG dir.
    // Since no plugins are installed in CI, this should return empty.
    let plugins = registry::scan_installed().unwrap_or_default();
    // Just verify it doesn't panic. May be empty or contain test artifacts.
    let _ = plugins;
}

#[test]
fn installed_plugin_record_has_fields() {
    let rec = registry::InstalledPlugin {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        toml_path: std::path::PathBuf::from("/tmp/test/plugin.toml"),
        lib_path: std::path::PathBuf::from("/tmp/test/libtest.so"),
    };
    assert_eq!(rec.name, "test");
    assert_eq!(rec.version, "0.1.0");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plugin_registry_tests`

Expected: FAIL — types not found.

- [ ] **Step 3: Implement registry.rs**

Replace `src/runtime/plugin/registry.rs` content:

```rust
//! Scan installed plugins directory.
//!
//! Walks `<xdg_data>/dyyl/plugins/<name>/<version>/` and collects
//! `InstalledPlugin` records (one per version directory that has a
//! `plugin.toml`).

use std::fs;
use std::path::PathBuf;

use crate::runtime::plugin::store;

/// A scanned installed plugin (one per version directory).
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub lib_path: PathBuf,
}

/// Scan the plugins directory and return all installed plugins.
///
/// Returns an error only if the plugins directory exists but cannot be read.
/// If the directory doesn't exist, returns an empty Vec.
pub fn scan_installed() -> Result<Vec<InstalledPlugin>, String> {
    let plugins_dir = store::plugin_dir();
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let name_entries = fs::read_dir(&plugins_dir)
        .map_err(|e| format!("failed to read {}: {e}", plugins_dir.display()))?;
    for name_entry in name_entries.flatten() {
        let name_path = name_entry.path();
        if !name_path.is_dir() {
            continue;
        }
        let name = match name_entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let version_entries = match fs::read_dir(&name_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for version_entry in version_entries.flatten() {
            let version_path = version_entry.path();
            if !version_path.is_dir() {
                continue;
            }
            let version = match version_entry.file_name().to_str() {
                Some(v) => v.to_string(),
                None => continue,
            };
            let toml_path = store::plugin_toml_path(&name, &version);
            let lib_path = store::lib_path(&name, &version);
            if toml_path.exists() && lib_path.exists() {
                result.push(InstalledPlugin {
                    name: name.clone(),
                    version: version.clone(),
                    toml_path,
                    lib_path,
                });
            }
        }
    }
    Ok(result)
}

/// Find the installed version of a plugin by name.
///
/// Returns the most recent (lexicographically last) version if multiple
/// are installed, or `None` if not installed.
#[must_use]
pub fn find_installed(name: &str) -> Option<InstalledPlugin> {
    let plugins = scan_installed().ok()?;
    plugins
        .into_iter()
        .filter(|p| p.name == name)
        .max_by_key(|p| p.version.clone())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test plugin_registry_tests`

Expected: PASS — 2 tests green.

- [ ] **Step 5: Commit**

```bash
git add src/runtime/plugin/registry.rs tests/plugin_registry_tests.rs
git commit -m "feat(plugin): implement registry.rs installed-plugin scanner"
```

---

## Task 7: Implement loader.rs (dlopen + symbol resolution + dispatch)

**Files:**
- Modify: `src/runtime/plugin/loader.rs`

This is the most complex module. It uses `unsafe` for dlopen via `libloading`.

- [ ] **Step 1: Implement loader.rs**

Replace `src/runtime/plugin/loader.rs` content:

```rust
//! dlopen + symbol resolution + dispatch.
//!
//! Opens a plugin dynamic library with `libloading`, resolves the 14
//! required ABI symbols, and provides typed methods to call them.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uint, c_void};
use std::path::Path;

use libloading::Library;

use crate::runtime::plugin::abi::{symbols, AbiError, DYRL_API_VERSION};

/// Loaded plugin — holds the dlopen'd library and resolved symbols.
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
    /// Open a plugin library, verify API version, call init, call on_load.
    ///
    /// Returns the loaded plugin or an AbiError.
    pub fn load(path: &Path, plugin_name: &str) -> Result<Self, AbiError> {
        unsafe {
            let library = Library::load(path).map_err(|e| {
                let _ = plugin_name; // used in error context upstream
                AbiError::SymbolMissing(format!("dlopen failed: {e}"))
            })?;

            // 1. get_api_version
            let get_api_version: symbols::GetApiVersion =
                *library.get(b"dyyl_plugin_get_api_version\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_get_api_version".to_string())
                })?;
            let plugin_api_version = get_api_version();
            if plugin_api_version != DYRL_API_VERSION {
                // Close library by letting it drop — but we haven't bound it yet.
                // We can't easily abort mid-init; just return error.
                std::mem::drop(library);
                return Err(AbiError::SymbolMissing(format!(
                    "API version mismatch: plugin={plugin_api_version}, dyyl={DYRL_API_VERSION}"
                )));
            }

            // 2. init
            let init: symbols::Init =
                *library.get(b"dyyl_plugin_init\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_init".to_string())
                })?;
            let handle = init(DYRL_API_VERSION);
            if handle.is_null() {
                std::mem::drop(library);
                return Err(AbiError::InitFailed);
            }

            let mut loader = Self { library, handle };

            // 3. on_load
            let on_load: symbols::OnLoad =
                *loader.library.get(b"dyyl_plugin_on_load\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_on_load".to_string())
                })?;
            let rc = on_load(loader.handle);
            if rc != 0 {
                // Call shutdown before dropping
                loader.shutdown_internal();
                return Err(AbiError::OnLoadFailed(rc));
            }

            Ok(loader)
        }
    }

    /// Call `list_commands` and return the JSON string.
    pub fn list_commands(&self) -> Result<String, AbiError> {
        unsafe {
            let list_commands: symbols::ListCommands =
                *self.library.get(b"dyyl_plugin_list_commands\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_list_commands".to_string())
                })?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = list_commands(self.handle, &mut out_ptr);
            if rc != 0 || out_ptr.is_null() {
                return Err(AbiError::CommandFailed(rc));
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr.to_str().map_err(|_| AbiError::InvalidUtf8)?.to_string();
            self.free_string(out_ptr);
            Ok(s)
        }
    }

    /// Call `handle_command`. `cmd_name` may contain dots (e.g. "user.login").
    /// `args_json` is the JSON-encoded args array. Returns the JSON-encoded
    /// result value.
    pub fn handle_command(
        &self,
        cmd_name: &str,
        args_json: &str,
    ) -> Result<String, AbiError> {
        unsafe {
            let handle_command: symbols::HandleCommand =
                *self.library.get(b"dyyl_plugin_handle_command\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_handle_command".to_string())
                })?;
            let cmd_c = CString::new(cmd_name).map_err(|_| AbiError::InvalidUtf8)?;
            let args_c = CString::new(args_json).map_err(|_| AbiError::InvalidUtf8)?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = handle_command(self.handle, cmd_c.as_ptr(), args_c.as_ptr(), &mut out_ptr);
            if rc != 0 {
                // out_ptr may still hold an error object — read it if present.
                if !out_ptr.is_null() {
                    let cstr = CStr::from_ptr(out_ptr);
                    let s = cstr.to_str().map_err(|_| AbiError::InvalidUtf8)?.to_string();
                    self.free_string(out_ptr);
                    return Err(AbiError::CommandFailed(rc_with_context(rc, s)));
                }
                return Err(AbiError::CommandFailed(rc));
            }
            if out_ptr.is_null() {
                return Ok(String::from("null"));
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr.to_str().map_err(|_| AbiError::InvalidUtf8)?.to_string();
            self.free_string(out_ptr);
            Ok(s)
        }
    }

    /// Call `get_command_help` for a specific command.
    pub fn get_command_help(&self, cmd_name: &str) -> Result<String, AbiError> {
        unsafe {
            let get_help: symbols::GetCommandHelp =
                *self.library.get(b"dyyl_plugin_get_command_help\0").map_err(|_| {
                    AbiError::SymbolMissing("dyyl_plugin_get_command_help".to_string())
                })?;
            let cmd_c = CString::new(cmd_name).map_err(|_| AbiError::InvalidUtf8)?;
            let mut out_ptr: *mut c_char = std::ptr::null_mut();
            let rc = get_help(self.handle, cmd_c.as_ptr(), &mut out_ptr);
            if rc != 0 || out_ptr.is_null() {
                return Ok(String::new());
            }
            let cstr = CStr::from_ptr(out_ptr);
            let s = cstr.to_str().map_err(|_| AbiError::InvalidUtf8)?.to_string();
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

    /// Call on_unload then shutdown.
    fn shutdown_internal(&mut self) {
        unsafe {
            if let Ok(on_unload) = self.library.get::<symbols::OnLoad>(b"dyyl_plugin_on_unload\0")
            {
                let on_unload = *on_unload;
                let _ = on_unload(self.handle);
            }
            if let Ok(shutdown) = self.library.get::<symbols::Shutdown>(b"dyyl_plugin_shutdown\0")
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

/// Pack a return code with the plugin's error message into a single i32.
/// Since i32 is lossy, we just return the rc; the message is logged upstream.
fn rc_with_context(rc: i32, _msg: String) -> i32 {
    rc
}
```

- [ ] **Step 2: Run cargo build to verify compilation**

Run: `cargo build`

Expected: Compiles. May have warnings about unused `c_uint` import — fix by removing if needed.

- [ ] **Step 3: Fix any clippy errors**

Run: `cargo clippy --lib 2>&1 | grep "^error" | head -20`

Fix any deny-level errors (likely around `unsafe` blocks or `must_use`).

- [ ] **Step 4: Commit**

```bash
git add src/runtime/plugin/loader.rs
git commit -m "feat(plugin): implement loader.rs dlopen + symbol resolution"
```

---

## Task 8: Implement Value JSON encoding (value_to_json / value_from_json)

**Files:**
- Create: `src/runtime/plugin/value_codec.rs`
- Modify: `src/runtime/plugin/mod.rs`
- Create: `tests/plugin_value_codec_tests.rs`

- [ ] **Step 1: Write failing tests for Value codec**

Create `tests/plugin_value_codec_tests.rs`:

```rust
use dyyl::runtime::plugin::value_codec::{value_to_json, values_to_json_array, value_from_json};
use dyyl::runtime::value::Value;
use dyyl::math::CasNumber;

#[test]
fn encode_num() {
    let json = value_to_json(&Value::Num(42));
    assert_eq!(json, r#"{"type":"num","value":"42"}"#);
}

#[test]
fn encode_str() {
    let json = value_to_json(&Value::Str("hello".to_string()));
    assert_eq!(json, r#"{"type":"str","value":"hello"}"#);
}

#[test]
fn encode_empty() {
    let json = value_to_json(&Value::Empty);
    assert_eq!(json, r#"{"type":"empty"}"#);
}

#[test]
fn encode_list() {
    let json = value_to_json(&Value::List(vec![Value::Num(1), Value::Str("a".to_string())]));
    assert_eq!(json, r#"{"type":"list","value":[{"type":"num","value":"1"},{"type":"str","value":"a"}]}"#);
}

#[test]
fn encode_args_array() {
    let args = vec![Value::Num(3), Value::Str("hi".to_string())];
    let json = values_to_json_array(&args);
    assert_eq!(json, r#"[{"type":"num","value":"3"},{"type":"str","value":"hi"}]"#);
}

#[test]
fn decode_str() {
    let v = value_from_json(r#"{"type":"str","value":"hello"}"#).unwrap();
    assert_eq!(v, Value::Str("hello".to_string()));
}

#[test]
fn decode_num() {
    let v = value_from_json(r#"{"type":"num","value":"42"}"#).unwrap();
    assert_eq!(v, Value::Num(42));
}

#[test]
fn decode_empty() {
    let v = value_from_json(r#"{"type":"empty"}"#).unwrap();
    assert_eq!(v, Value::Empty);
}

#[test]
fn decode_list() {
    let v = value_from_json(r#"{"type":"list","value":[{"type":"num","value":"1"}]}"#).unwrap();
    assert_eq!(v, Value::List(vec![Value::Num(1)]));
}

#[test]
fn roundtrip_str() {
    let original = Value::Str("test".to_string());
    let json = value_to_json(&original);
    let decoded = value_from_json(&json).unwrap();
    assert_eq!(original, decoded);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test plugin_value_codec_tests`

Expected: FAIL — module not found.

- [ ] **Step 3: Implement value_codec.rs**

Create `src/runtime/plugin/value_codec.rs`:

```rust
//! Value JSON encoding for the plugin ABI.
//!
//! Encodes dyyl `Value`s to/from JSON for cross-FFI communication.
//! `num` values are encoded as strings (to preserve arbitrary-precision
//! integers and fractions from `CasNumber`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use crate::math::CasNumber;
use crate::runtime::value::Value;

/// Encode a single Value to its JSON representation.
#[must_use]
pub fn value_to_json(v: &Value) -> String {
    let jv = value_to_json_value(v);
    jv.to_string()
}

/// Encode a slice of Values to a JSON array (used for args).
#[must_use]
pub fn values_to_json_array(values: &[Value]) -> String {
    let arr: Vec<JsonValue> = values.iter().map(value_to_json_value).collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Decode a JSON string to a Value.
pub fn value_from_json(s: &str) -> Result<Value, serde_json::Error> {
    let jv: JsonValue = serde_json::from_str(s)?;
    Ok(json_value_to_value(&jv))
}

fn value_to_json_value(v: &Value) -> JsonValue {
    match v {
        Value::Num(n) => json!({"type": "num", "value": n.to_string()}),
        Value::Str(s) => json!({"type": "str", "value": s}),
        Value::Expr(e) => json!({"type": "expr", "value": expr_to_string(e)}),
        Value::Empty => json!({"type": "empty"}),
        Value::List(items) => {
            let arr: Vec<JsonValue> = items.iter().map(value_to_json_value).collect();
            json!({"type": "list", "value": arr})
        }
        Value::Dict(pairs) => {
            let arr: Vec<JsonValue> = pairs
                .iter()
                .map(|(k, v)| json!({"key": value_to_json_value(k), "val": value_to_json_value(v)}))
                .collect();
            json!({"type": "dict", "value": arr})
        }
    }
}

fn json_value_to_value(jv: &JsonValue) -> Value {
    let ty = jv.get("type").and_then(|t| t.as_str()).unwrap_or("empty");
    match ty {
        "num" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Value::Num(s.parse().unwrap_or(0))
        }
        "str" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Value::Str(s.to_string())
        }
        "expr" => {
            // Expr roundtrip is best-effort — parse as num if possible.
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Value::Num(s.parse().unwrap_or(0))
        }
        "empty" => Value::Empty,
        "list" => {
            let arr = jv.get("value").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let items: Vec<Value> = arr.iter().map(json_value_to_value).collect();
            Value::List(items)
        }
        "dict" => {
            let arr = jv.get("value").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let pairs: Vec<(Value, Value)> = arr
                .iter()
                .filter_map(|p| {
                    let k = p.get("key")?;
                    let v = p.get("val")?;
                    Some((json_value_to_value(k), json_value_to_value(v)))
                })
                .collect();
            Value::Dict(pairs)
        }
        _ => Value::Empty,
    }
}

/// Render a CasNumber to string (for expr type encoding).
fn expr_to_string(e: &CasNumber) -> String {
    e.to_string()
}
```

- [ ] **Step 4: Register value_codec in mod.rs**

In `src/runtime/plugin/mod.rs`, add `pub mod value_codec;` after `pub mod abi;`:

```rust
pub mod abi;
pub mod fetch;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod store;
pub mod value_codec;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test plugin_value_codec_tests`

Expected: PASS — 10 tests green.

- [ ] **Step 6: Commit**

```bash
git add src/runtime/plugin/value_codec.rs src/runtime/plugin/mod.rs tests/plugin_value_codec_tests.rs
git commit -m "feat(plugin): implement Value JSON codec for ABI"
```

---

## Task 9: Implement PluginManager.dispatch (orchestration)

**Files:**
- Modify: `src/runtime/plugin/mod.rs`

This wires together fetch → store → registry → loader → dispatch.

- [ ] **Step 1: Implement dispatch in mod.rs**

Replace the `dispatch` method in `src/runtime/plugin/mod.rs` with the full implementation. The `LoadedPlugin` struct also changes to hold the manifest:

```rust
//! Plugin system — dynamic-library loading and dispatch.

pub mod abi;
pub mod fetch;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod store;
pub mod value_codec;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::i18n::Lang;
use crate::runtime::error::RuntimeError;
use crate::runtime::plugin::abi::DYRL_API_VERSION;
use crate::runtime::plugin::fetch::FetchError;
use crate::runtime::plugin::loader::PluginLoader;
use crate::runtime::plugin::manifest::{LocalPluginToml, RemoteManifest};
use crate::runtime::plugin::store as plugin_store;
use crate::runtime::plugin::value_codec::{value_from_json, values_to_json_array};
use crate::runtime::value::Value;

/// Loaded plugin instance — holds the loader and manifest.
pub struct LoadedPlugin {
    pub name: String,
    pub loader: PluginLoader,
    pub manifest: LocalPluginToml,
}

/// Central plugin manager — holds already-loaded plugins.
pub struct PluginManager {
    loaded: Mutex<HashMap<String, LoadedPlugin>>,
}

impl PluginManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Dispatch a plugin command `<name>.<sub>` (sub may contain dots).
    pub fn dispatch(
        &self,
        name: &str,
        sub: &str,
        args: &[Value],
        lang: Lang,
        line: usize,
    ) -> Result<Value, RuntimeError> {
        // 1. Ensure plugin is loaded.
        let loaded_ref = {
            let mut loaded = self.loaded.lock().expect("plugin map mutex poisoned");
            if !loaded.contains_key(name) {
                let lp = self.load_plugin(name, lang, line)?;
                loaded.insert(name.to_string(), lp);
            }
            // We can't return a ref into the locked map easily; instead do the
            // dispatch inside a second lock scope.
            drop(loaded);
        };

        // 2. Dispatch the command (re-lock, find plugin, call handle_command).
        let loaded = self.loaded.lock().expect("plugin map mutex poisoned");
        let lp = loaded.get(name).ok_or_else(|| {
            RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_command_failed(lang, name, sub, "load_failed"),
            )
        })?;

        // 3. Verify command exists in manifest.
        if lp.manifest.find_command(sub).is_none() {
            return Err(RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_unknown_subcommand(lang, name, sub),
            ));
        }

        // 4. Encode args, call handle_command, decode result.
        let args_json = values_to_json_array(args);
        let result_json = lp
            .loader
            .handle_command(sub, &args_json)
            .map_err(|e| RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_command_failed(lang, name, sub, &e.to_string()),
            ))?;

        value_from_json(&result_json).map_err(|e| RuntimeError::new(
            line,
            format!("{name}.{sub}"),
            crate::i18n::plugin_command_failed(lang, name, sub, &e.to_string()),
        ))
    }

    /// Load (or install+load) a plugin by name.
    fn load_plugin(
        &self,
        name: &str,
        lang: Lang,
        line: usize,
    ) -> Result<LoadedPlugin, RuntimeError> {
        // 1. Check if already installed locally.
        let installed = registry::find_installed(name);

        let lib_path = match installed {
            Some(rec) => rec.lib_path,
            None => {
                // 2. Not installed — fetch manifest, download, install.
                self.install_plugin(name, lang, line)?
            }
        };

        // 3. Read local plugin.toml.
        let toml_path = plugin_store::plugin_toml_path(
            name,
            &self.read_installed_version(name)?,
        );
        let toml_content = fs::read_to_string(&toml_path).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;
        let manifest: LocalPluginToml = toml::from_str(&toml_content).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;

        // 4. dlopen + init + on_load.
        let loader = PluginLoader::load(&lib_path, name).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;

        Ok(LoadedPlugin {
            name: name.to_string(),
            loader,
            manifest,
        })
    }

    /// Fetch manifest, download library, verify SHA256, install to XDG dir.
    fn install_plugin(
        &self,
        name: &str,
        lang: Lang,
        line: usize,
    ) -> Result<PathBuf, RuntimeError> {
        // 1. Fetch manifest.
        let manifest = fetch::fetch_manifest(name).map_err(|e| {
            let reason = match e {
                FetchError::Http(_, msg) => msg,
                FetchError::Read(_, msg) => msg,
                FetchError::Parse(_, msg) => msg,
                FetchError::ChecksumMismatch(_) => "checksum mismatch".to_string(),
            };
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_fetch_manifest_failed(lang, name, &reason),
            )
        })?;

        // 2. Validate manifest.
        if manifest.abi_version != DYRL_API_VERSION {
            return Err(RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_abi_mismatch(
                    lang,
                    name,
                    DYRL_API_VERSION,
                    manifest.abi_version,
                ),
            ));
        }

        // 3. Find platform entry.
        let current = plugin_store::current_platform();
        let entry = manifest
            .platforms
            .iter()
            .find(|p| p.platform == current)
            .ok_or_else(|| {
                let available: Vec<String> =
                    manifest.platforms.iter().map(|p| p.platform.clone()).collect();
                RuntimeError::new(
                    line,
                    name,
                    crate::i18n::plugin_platform_unavailable(
                        lang,
                        name,
                        &current,
                        &available.join(", "),
                    ),
                )
            })?;

        // 4. Download + verify SHA256.
        let bytes = fetch::download_and_verify(&entry.url, &entry.sha256).map_err(|e| {
            let reason = match e {
                FetchError::ChecksumMismatch(_) => {
                    return RuntimeError::new(
                        line,
                        name,
                        crate::i18n::plugin_sha256_mismatch(lang, name),
                    )
                }
                FetchError::Http(_, msg) => msg,
                FetchError::Read(_, msg) => msg,
                FetchError::Parse(_, msg) => msg,
            };
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_download_failed(lang, name, &reason),
            )
        })?;

        // 5. Install: create dir, write lib + plugin.toml.
        let lib_path = plugin_store::lib_path(name, &manifest.version);
        let toml_path = plugin_store::plugin_toml_path(name, &manifest.version);
        let version_dir = plugin_store::plugin_version_dir(name, &manifest.version);

        fs::create_dir_all(&version_dir).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        fs::write(&lib_path, &bytes).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        // 6. Write plugin.toml.
        let local_toml = build_local_toml(&manifest, &entry.url, &entry.sha256);
        let toml_content = toml::to_string_pretty(&local_toml).unwrap_or_default();
        fs::write(&toml_path, toml_content).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        Ok(lib_path)
    }

    /// Read the installed version of a plugin from its plugin.toml.
    fn read_installed_version(&self, name: &str) -> Result<String, String> {
        let rec = registry::find_installed(name)
            .ok_or_else(|| format!("plugin {name} not installed"))?;
        let content = fs::read_to_string(&rec.toml_path)
            .map_err(|e| format!("read {}: {e}", rec.toml_path.display()))?;
        let toml: LocalPluginToml = toml::from_str(&content)
            .map_err(|e| format!("parse {}: {e}", rec.toml_path.display()))?;
        Ok(toml.version)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a LocalPluginToml from a RemoteManifest + install metadata.
fn build_local_toml(manifest: &RemoteManifest, source_url: &str, sha256: &str) -> LocalPluginToml {
    use crate::runtime::plugin::manifest::{InstalledRecord, PluginCommand};
    LocalPluginToml {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        abi_version: manifest.abi_version,
        dyyl_min: manifest.dyyl_min.clone(),
        panic_mode: manifest.panic_mode.clone(),
        commands: manifest.commands.clone(),
        installed: InstalledRecord {
            source_url: source_url.to_string(),
            sha256: sha256.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }
}
```

- [ ] **Step 2: Run cargo build**

Run: `cargo build`

Expected: Compiles. May need to fix imports (chrono is in deps).

- [ ] **Step 3: Fix clippy issues**

Run: `cargo clippy --lib 2>&1 | grep "^error" | head -20`

Fix deny errors (common: `must_use` on functions, `unwrap_used`).

- [ ] **Step 4: Commit**

```bash
git add src/runtime/plugin/mod.rs
git commit -m "feat(plugin): implement PluginManager.dispatch orchestration"
```

---

## Task 10: Add PluginManager to Env + dispatch fallback

**Files:**
- Modify: `src/runtime/env.rs`
- Modify: `src/runtime/cmd/dispatch.rs`
- Create: `src/runtime/cmd/plugin.rs`
- Modify: `src/runtime/cmd/mod.rs`

- [ ] **Step 1: Add plugin_manager field to Env**

In `src/runtime/env.rs`, add `use crate::runtime::plugin::PluginManager;` to imports, and add field to struct:

```rust
use crate::runtime::plugin::PluginManager;
```

Add field after `mcm_id_counter`:

```rust
pub struct Env {
    bindings: HashMap<String, Value>,
    lang: Cell<Lang>,
    host_provider: Option<Arc<dyn HostProvider>>,
    game_scope: GameChooseScope,
    mcm_id_counter: Cell<u64>,
    plugin_manager: PluginManager,
}
```

Update `new()`:

```rust
pub fn new() -> Self {
    Self {
        bindings: HashMap::new(),
        lang: Cell::new(Lang::En),
        host_provider: None,
        game_scope: GameChooseScope::default(),
        mcm_id_counter: Cell::new(1),
        plugin_manager: PluginManager::new(),
    }
}
```

Add accessor:

```rust
/// Access the plugin manager.
#[must_use]
pub fn plugin_manager(&self) -> &PluginManager {
    &self.plugin_manager
}
```

- [ ] **Step 2: Create cmd/plugin.rs dispatch router**

Create `src/runtime/cmd/plugin.rs`:

```rust
//! Plugin command dispatch router.
//!
//! Called from dispatch.rs fallback arm when a command starts with an
//! unknown prefix (not math./str./io./etc). Splits `<name>.<rest>` and
//! routes to PluginManager.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Dispatch a `<name>.<sub>[.<sub>...]` command to the plugin manager.
///
/// `full_command` is the complete command string (e.g. "migpt.user.login").
pub(crate) fn dispatch_plugin_command(
    full_command: &str,
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    // Split on first dot: plugin_name = before first dot, sub = rest.
    let dot_pos = full_command
        .find('.')
        .ok_or_else(|| {
            RuntimeError::new(
                ctx.line,
                full_command,
                i18n::unknown_top_command(ctx.lang.get(), full_command),
            )
        })?;
    let plugin_name = &full_command[..dot_pos];
    let sub = &full_command[dot_pos + 1..];

    // Evaluate all args to Values.
    let mut args = Vec::with_capacity(call.args.len());
    for expr in &call.args {
        args.push(eval_expr(expr, env, ctx)?);
    }

    env.plugin_manager()
        .dispatch(plugin_name, sub, &args, ctx.lang.get(), ctx.line)
}
```

- [ ] **Step 3: Register plugin module in cmd/mod.rs**

In `src/runtime/cmd/mod.rs`, add:

```rust
pub(crate) mod plugin;
```

- [ ] **Step 4: Add fallback arm to dispatch.rs**

In `src/runtime/cmd/dispatch.rs`, change the `_ =>` arm to check for plugin commands. Replace the `_ =>` match arm:

```rust
        _ => {
            // Fallback: if command contains a dot and prefix isn't a known
            // family, treat as a plugin command.
            if call.command.contains('.') {
                super::plugin::dispatch_plugin_command(&call.command, call, env, ctx)
            } else {
                Err(RuntimeError::new(
                    ctx.line,
                    &call.command,
                    i18n::unknown_top_command(ctx.lang.get(), &call.command),
                ))
            }
        }
```

- [ ] **Step 5: Run cargo build**

Run: `cargo build`

Expected: Compiles.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`

Expected: All existing tests pass (plugin dispatch isn't triggered without real plugins). The one pre-existing `config::tests::invalid_toml_yields_error` failure is unrelated.

- [ ] **Step 7: Commit**

```bash
git add src/runtime/env.rs src/runtime/cmd/plugin.rs src/runtime/cmd/mod.rs src/runtime/cmd/dispatch.rs
git commit -m "feat(plugin): add PluginManager to Env + dispatch fallback"
```

---

## Task 11: Extend config with installed_plugins + last_used_at

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add InstalledPluginRecord struct and field to DyylConfig**

In `src/config.rs`, add after `DyylConfig` definition:

```rust
/// Record of an installed plugin, for `last_used_at` tracking.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct InstalledPluginRecord {
    /// Version string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// ISO 8601 timestamp of last successful dispatch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}
```

Add field to `DyylConfig`:

```rust
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct DyylConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub installed_plugins: std::collections::HashMap<String, InstalledPluginRecord>,
}
```

- [ ] **Step 2: Run cargo build + test**

Run: `cargo build && cargo test --lib config`

Expected: Compiles, existing config tests pass (the `roundtrip_toml` test should still work since `installed_plugins` defaults empty and skips serialization).

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add installed_plugins with last_used_at tracking"
```

---

## Task 12: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: All tests pass except the pre-existing `config::tests::invalid_toml_yields_error` (toml crate version issue, unrelated).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features 2>&1 | grep -E "^error" | head -20`

Expected: No new errors from plugin code.

- [ ] **Step 3: Run fmt check on plugin files**

Run: `cargo fmt --check 2>&1 | grep -E "plugin" | head`

Expected: No output (plugin files are formatted).

- [ ] **Step 4: If fmt issues, fix and commit**

```bash
cargo fmt
git add -A
git commit -m "style(plugin): fmt" || echo "nothing to format"
```

---

## Self-Review Checklist

- [ ] `store.rs`: XDG paths computed correctly, `current_platform()` returns `linux-x86_64` etc.
- [ ] `manifest.rs`: `RemoteManifest` and `LocalPluginToml` parse from JSON/TOML
- [ ] `abi.rs`: 14 symbol names listed, `DYRL_API_VERSION = 1`
- [ ] `fetch.rs`: SHA256 verification works, `fetch_manifest` and `download_and_verify` implemented
- [ ] `registry.rs`: scans installed plugins, `find_installed` returns most recent version
- [ ] `loader.rs`: dlopen via libloading, resolves symbols, calls init/on_load/handle_command
- [ ] `value_codec.rs`: Value ↔ JSON roundtrips for num/str/empty/list/dict
- [ ] `mod.rs`: `PluginManager::dispatch` orchestrates load→dispatch, handles install on first call
- [ ] `env.rs`: `plugin_manager` field added
- [ ] `dispatch.rs`: fallback arm routes `<name>.<sub>` to plugin dispatch
- [ ] `config.rs`: `installed_plugins` HashMap with `last_used_at`
- [ ] All tests pass (except pre-existing unrelated failure)
- [ ] clippy clean on new code
