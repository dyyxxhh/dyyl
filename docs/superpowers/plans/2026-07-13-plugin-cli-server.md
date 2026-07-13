# Plugin CLI + Server — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Add CLI subcommands (`dyyl install/update/remove/autoremove/list`) and extend `server.js` with plugin distribution routes. Includes a test fixture cdylib for integration testing.

**Architecture:** New `src/cli/` module with `plugin_cmds.rs` implementing the 5 subcommands, reusing Phase 1's `fetch.rs` + `store.rs` + `registry.rs`. `main.rs` gets subcommand dispatch before the existing flag parsing. `server.js` gets `/plugins/<name>/manifest.json` and `/plugins/<name>/<version>/<platform>/<filename>` routes. A `tests/fixtures/example-plugin/` cdylib provides a real plugin for end-to-end tests.

**Tech Stack:** Rust 2021, existing deps. server.js is Node.js (existing).

**Spec:** [docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md) §9 (CLI), §8 (server)

---

## File Structure

| File | Responsibility |
|---|---|
| `src/cli/mod.rs` (create) | CLI subcommand dispatch entry |
| `src/cli/plugin_cmds.rs` (create) | install/update/remove/autoremove/list implementations |
| `src/main.rs` (modify) | Detect subcommands before flag parsing |
| `server.js` (modify) | Add `/plugins/...` routes |
| `scripts/publish-plugin.sh` (create) | Pack+hash+generate manifest |
| `tests/fixtures/example-plugin/Cargo.toml` (create) | cdylib test fixture |
| `tests/fixtures/example-plugin/src/lib.rs` (create) | Plugin impl for tests |
| `tests/plugin_cli_tests.rs` (create) | CLI integration tests |

---

## Task 1: Create CLI module skeleton + main.rs subcommand detection

**Files:**
- Create: `src/cli/mod.rs`
- Create: `src/cli/plugin_cmds.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create src/cli/mod.rs**

```rust
//! CLI subcommand dispatch for plugin management.
//!
//! Entry point for `dyyl install|update|remove|autoremove|list`.

pub mod plugin_cmds;

use crate::i18n::Lang;

/// Outcome of CLI subcommand handling.
pub enum CliResult {
    /// Subcommand handled, exit with this code.
    Handled(i32),
    /// Not a subcommand — continue with normal script execution.
    NotASubcommand,
}

/// Check if args[1] is a known subcommand; if so, handle it.
///
/// Global options like `--lang` must come before the subcommand.
pub fn try_handle_subcommand(args: &[String], lang: &mut Lang) -> CliResult {
    // Find first non-flag arg (the subcommand candidate).
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--lang" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    if let Some(l) = Lang::from_name(val) {
                        *lang = l;
                    }
                }
            }
            "--debug" => { /* ignore for CLI */ }
            "install" | "update" | "remove" | "autoremove" | "list" => {
                let sub = args[i].as_str();
                let rest = &args[i + 1..];
                let code = plugin_cmds::dispatch(sub, rest, *lang);
                return CliResult::Handled(code);
            }
            _ => return CliResult::NotASubcommand,
        }
        i += 1;
    }
    CliResult::NotASubcommand
}
```

- [ ] **Step 2: Create src/cli/plugin_cmds.rs stub**

```rust
//! Plugin CLI subcommands: install, update, remove, autoremove, list.

use crate::i18n::Lang;

/// Dispatch a plugin subcommand. Returns exit code.
pub fn dispatch(sub: &str, args: &[String], lang: Lang) -> i32 {
    match sub {
        "install" => cmd_install(args, lang),
        "update" => cmd_update(args, lang),
        "remove" => cmd_remove(args, lang),
        "autoremove" => cmd_autoremove(args, lang),
        "list" => cmd_list(args, lang),
        _ => {
            eprintln!("{}", crate::i18n::cli_plugin_subcommand_unknown(lang, sub));
            1
        }
    }
}

fn cmd_install(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("install: not yet implemented");
    1
}

fn cmd_update(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("update: not yet implemented");
    1
}

fn cmd_remove(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("remove: not yet implemented");
    1
}

fn cmd_autoremove(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("autoremove: not yet implemented");
    1
}

fn cmd_list(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("list: not yet implemented");
    1
}
```

- [ ] **Step 3: Register cli module in lib.rs**

In `src/lib.rs`, add `pub mod cli;` after `pub mod config;`:

```rust
pub mod cas_backend;
pub mod cli;
pub mod config;
pub mod i18n;
```

- [ ] **Step 4: Wire subcommand detection into main.rs**

In `src/main.rs`, at the very start of `fn main()` (before the existing arg parsing), add:

```rust
fn main() {
    let args: Vec<String> = env::args().collect();

    // Check for plugin management subcommands first.
    let mut lang = Lang::default();
    match dyyl::cli::try_handle_subcommand(&args, &mut lang) {
        dyyl::cli::CliResult::Handled(code) => process::exit(code),
        dyyl::cli::CliResult::NotASubcommand => {}
    }

    // Existing arg parsing continues here...
    let mut debug = false;
    // ... (rest of existing main unchanged)
```

- [ ] **Step 5: Run cargo build**

Run: `cargo build`

Expected: Compiles.

- [ ] **Step 6: Verify subcommand detection works**

Run: `cargo run -- install 2>&1 | head -3`

Expected: `install: not yet implemented`

Run: `cargo run -- list 2>&1 | head -3`

Expected: `list: not yet implemented`

- [ ] **Step 7: Commit**

```bash
git add src/cli/ src/lib.rs src/main.rs
git commit -m "feat(cli): add plugin subcommand skeleton + main.rs detection"
```

---

## Task 2: Implement `dyyl list` command

**Files:**
- Modify: `src/cli/plugin_cmds.rs`

- [ ] **Step 1: Implement cmd_list**

Replace the `cmd_list` stub in `src/cli/plugin_cmds.rs`:

```rust
fn cmd_list(args: &[String], lang: Lang) -> i32 {
    let _ = args;
    let installed = match dyyl::runtime::plugin::registry::scan_installed() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if installed.is_empty() {
        println!("{}", crate::i18n::plugin_list_empty(lang));
        return 0;
    }
    // Header
    println!("{}", crate::i18n::plugin_list_header(lang));
    for p in &installed {
        // Read plugin.toml for last_used_at.
        let toml_content = match std::fs::read_to_string(&p.toml_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let toml: dyyl::runtime::plugin::manifest::LocalPluginToml = match toml::from_str(&toml_content) {
            Ok(t) => t,
            Err(_) => continue,
        };
        // Check config for last_used_at.
        let last_used = dyyl::config::load_config()
            .ok()
            .and_then(|c| c.installed_plugins.get(&p.name).and_then(|r| r.last_used_at.clone()))
            .unwrap_or_else(|| "-".to_string());
        println!("{} {} {} {}", p.name, p.version, last_used, toml.installed.installed_at);
    }
    0
}
```

- [ ] **Step 2: Run cargo build + test**

Run: `cargo build && cargo run -- list`

Expected: Prints `no plugins installed` (or `未安装任何插件` with `--lang zh`).

- [ ] **Step 3: Commit**

```bash
git add src/cli/plugin_cmds.rs
git commit -m "feat(cli): implement dyyl list command"
```

---

## Task 3: Implement `dyyl install <name>` command

**Files:**
- Modify: `src/cli/plugin_cmds.rs`

- [ ] **Step 1: Implement cmd_install**

Replace the `cmd_install` stub:

```rust
fn cmd_install(args: &[String], lang: Lang) -> i32 {
    if args.is_empty() {
        eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
        return 1;
    }
    let name = &args[0];

    // Check if already installed with same version.
    if let Some(existing) = dyyl::runtime::plugin::registry::find_installed(name) {
        let toml_content = match std::fs::read_to_string(&existing.toml_path) {
            Ok(s) => s,
            Err(_) => String::new(),
        };
        if let Ok(t) = toml::from_str::<dyyl::runtime::plugin::manifest::LocalPluginToml>(&toml_content) {
            // Fetch remote manifest to check if same version.
            match dyyl::runtime::plugin::fetch::fetch_manifest(name) {
                Ok(remote) if remote.version == t.version => {
                    println!("{}", crate::i18n::plugin_already_installed(lang, name, &t.version));
                    return 0;
                }
                _ => {}
            }
        }
    }

    // Install via fetch + download + verify + write.
    match install_plugin_by_name(name, lang) {
        Ok(version) => {
            println!("{}", crate::i18n::plugin_install_success(lang, name, &version));
            0
        }
        Err(e) => {
            eprintln!("{}", crate::i18n::plugin_install_failed(lang, name, &e));
            1
        }
    }
}

/// Install a plugin: fetch manifest, download, verify, write to XDG dir.
/// Returns the installed version string.
fn install_plugin_by_name(name: &str, lang: Lang) -> Result<String, String> {
    use dyyl::runtime::plugin::{abi::DYRL_API_VERSION, fetch, manifest::*, store};
    use std::fs;

    let manifest = fetch::fetch_manifest(name)
        .map_err(|e| format!("{e}"))?;

    if manifest.abi_version != DYRL_API_VERSION {
        return Err(crate::i18n::plugin_abi_mismatch(
            lang, name, DYRL_API_VERSION, manifest.abi_version,
        ));
    }

    let current = store::current_platform();
    let entry = manifest.platforms.iter()
        .find(|p| p.platform == current)
        .ok_or_else(|| format!("no build for {current}"))?;

    let bytes = fetch::download_and_verify(&entry.url, &entry.sha256)
        .map_err(|e| format!("{e}"))?;

    let lib_path = store::lib_path(name, &manifest.version);
    let toml_path = store::plugin_toml_path(name, &manifest.version);
    let version_dir = store::plugin_version_dir(name, &manifest.version);

    fs::create_dir_all(&version_dir).map_err(|e| format!("{e}"))?;
    fs::write(&lib_path, &bytes).map_err(|e| format!("{e}"))?;

    // Build local toml.
    let local_toml = LocalPluginToml {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        abi_version: manifest.abi_version,
        dyyl_min: manifest.dyyl_min.clone(),
        panic_mode: manifest.panic_mode.clone(),
        commands: manifest.commands.clone(),
        installed: InstalledRecord {
            source_url: entry.url.clone(),
            sha256: entry.sha256.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    let toml_content = toml::to_string_pretty(&local_toml).unwrap_or_default();
    fs::write(&toml_path, toml_content).map_err(|e| format!("{e}"))?;

    Ok(manifest.version)
}
```

- [ ] **Step 2: Run cargo build**

Run: `cargo build`

Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add src/cli/plugin_cmds.rs
git commit -m "feat(cli): implement dyyl install command"
```

---

## Task 4: Implement `dyyl remove <name>` command

**Files:**
- Modify: `src/cli/plugin_cmds.rs`

- [ ] **Step 1: Implement cmd_remove**

Replace the `cmd_remove` stub:

```rust
fn cmd_remove(args: &[String], lang: Lang) -> i32 {
    if args.is_empty() {
        eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
        return 1;
    }
    let name = &args[0];

    // Check if installed.
    let installed = match dyyl::runtime::plugin::registry::find_installed(name) {
        Some(p) => p,
        None => {
            eprintln!("{}", crate::i18n::plugin_not_installed(lang, name));
            return 1;
        }
    };

    // Remove the entire plugin directory (all versions).
    let plugin_dir = dyyl::runtime::plugin::store::plugin_dir().join(name);
    if let Err(e) = std::fs::remove_dir_all(&plugin_dir) {
        eprintln!("{}", crate::i18n::plugin_remove_failed(lang, name, &e.to_string()));
        return 1;
    }

    // Remove from config.
    if let Ok(mut config) = dyyl::config::load_config() {
        config.installed_plugins.remove(name);
        let _ = dyyl::config::save_config(&config);
    }

    println!("{}", crate::i18n::plugin_removed(lang, name));
    0
}
```

- [ ] **Step 2: Commit**

```bash
git add src/cli/plugin_cmds.rs
git commit -m "feat(cli): implement dyyl remove command"
```

---

## Task 5: Implement `dyyl update [name]` command

**Files:**
- Modify: `src/cli/plugin_cmds.rs`

- [ ] **Step 1: Implement cmd_update**

Replace the `cmd_update` stub:

```rust
fn cmd_update(args: &[String], lang: Lang) -> i32 {
    if args.is_empty() {
        // Update all installed plugins.
        let installed = match dyyl::runtime::plugin::registry::scan_installed() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        let mut updated = 0usize;
        let mut latest = 0usize;
        let mut failed = 0usize;
        for p in &installed {
            match update_single(&p.name, lang) {
                UpdateOutcome::Updated => updated += 1,
                UpdateOutcome::AlreadyLatest => latest += 1,
                UpdateOutcome::Failed(_) => failed += 1,
            }
        }
        println!(
            "{}",
            crate::i18n::plugin_update_all_summary(lang, updated, latest, failed)
        );
        if failed > 0 { 1 } else { 0 }
    } else {
        let name = &args[0];
        if dyyl::runtime::plugin::registry::find_installed(name).is_none() {
            eprintln!("{}", crate::i18n::plugin_not_installed(lang, name));
            return 1;
        }
        match update_single(name, lang) {
            UpdateOutcome::Updated => 0,
            UpdateOutcome::AlreadyLatest => 0,
            UpdateOutcome::Failed(e) => {
                eprintln!("{}", crate::i18n::plugin_update_failed(lang, name, &e));
                1
            }
        }
    }
}

enum UpdateOutcome {
    Updated,
    AlreadyLatest,
    Failed(String),
}

fn update_single(name: &str, lang: Lang) -> UpdateOutcome {
    use dyyl::runtime::plugin::{fetch, registry, store};
    use std::fs;

    // Get current installed version.
    let current = match registry::find_installed(name) {
        Some(p) => p,
        None => return UpdateOutcome::Failed("not installed".to_string()),
    };
    let toml_content = match fs::read_to_string(&current.toml_path) {
        Ok(s) => s,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };
    let local: dyyl::runtime::plugin::manifest::LocalPluginToml = match toml::from_str(&toml_content) {
        Ok(t) => t,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    // Fetch remote manifest.
    let remote = match fetch::fetch_manifest(name) {
        Ok(m) => m,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    if remote.version == local.version {
        return UpdateOutcome::AlreadyLatest;
    }

    // Download + install new version.
    let cur_platform = store::current_platform();
    let entry = match remote.platforms.iter().find(|p| p.platform == cur_platform) {
        Some(e) => e,
        None => return UpdateOutcome::Failed(format!("no build for {cur_platform}")),
    };

    let bytes = match fetch::download_and_verify(&entry.url, &entry.sha256) {
        Ok(b) => b,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    let new_lib_path = store::lib_path(name, &remote.version);
    let new_toml_path = store::plugin_toml_path(name, &remote.version);
    let new_version_dir = store::plugin_version_dir(name, &remote.version);

    if let Err(e) = fs::create_dir_all(&new_version_dir) {
        return UpdateOutcome::Failed(e.to_string());
    }
    if let Err(e) = fs::write(&new_lib_path, &bytes) {
        return UpdateOutcome::Failed(e.to_string());
    }

    let new_local = dyyl::runtime::plugin::manifest::LocalPluginToml {
        name: remote.name.clone(),
        version: remote.version.clone(),
        abi_version: remote.abi_version,
        dyyl_min: remote.dyyl_min.clone(),
        panic_mode: remote.panic_mode.clone(),
        commands: remote.commands.clone(),
        installed: dyyl::runtime::plugin::manifest::InstalledRecord {
            source_url: entry.url.clone(),
            sha256: entry.sha256.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    let toml_content = toml::to_string_pretty(&new_local).unwrap_or_default();
    if let Err(e) = fs::write(&new_toml_path, toml_content) {
        return UpdateOutcome::Failed(e.to_string());
    }

    // Remove old version directory.
    let old_version_dir = store::plugin_version_dir(name, &local.version);
    let _ = fs::remove_dir_all(&old_version_dir);

    println!(
        "{}",
        crate::i18n::plugin_updated(lang, name, &local.version, &remote.version)
    );
    UpdateOutcome::Updated
}
```

- [ ] **Step 2: Commit**

```bash
git add src/cli/plugin_cmds.rs
git commit -m "feat(cli): implement dyyl update command"
```

---

## Task 6: Implement `dyyl autoremove` command

**Files:**
- Modify: `src/cli/plugin_cmds.rs`

- [ ] **Step 1: Add autoremove_days const and implement cmd_autoremove**

Add at top of `src/cli/plugin_cmds.rs`:

```rust
/// Plugins unused for this many days are removed by `autoremove`.
const AUTOREMOVE_DAYS: i64 = 30;
```

Replace the `cmd_autoremove` stub:

```rust
fn cmd_autoremove(args: &[String], lang: Lang) -> i32 {
    let _ = args;
    let installed = match dyyl::runtime::plugin::registry::scan_installed() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let config = dyyl::config::load_config().unwrap_or_default();
    let now = chrono::Utc::now();
    let mut removed_count = 0usize;
    let mut config = config;

    for p in &installed {
        let last_used = config
            .installed_plugins
            .get(&p.name)
            .and_then(|r| r.last_used_at.as_ref())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let days_ago: i64 = match last_used {
            Some(dt) => (now - dt).num_days(),
            None => i64::MAX, // Never used — remove.
        };

        if days_ago >= AUTOREMOVE_DAYS {
            let plugin_dir = dyyl::runtime::plugin::store::plugin_dir().join(&p.name);
            if std::fs::remove_dir_all(&plugin_dir).is_ok() {
                config.installed_plugins.remove(&p.name);
                if days_ago == i64::MAX {
                    println!("{}", crate::i18n::plugin_removed(lang, &p.name));
                } else {
                    println!(
                        "{}",
                        crate::i18n::plugin_autoremove_removed(lang, &p.name, days_ago as u64)
                    );
                }
                removed_count += 1;
            }
        }
    }

    if removed_count > 0 {
        let _ = dyyl::config::save_config(&config);
    }
    println!(
        "{}",
        crate::i18n::plugin_autoremove_summary(lang, removed_count)
    );
    0
}
```

- [ ] **Step 2: Commit**

```bash
git add src/cli/plugin_cmds.rs
git commit -m "feat(cli): implement dyyl autoremove command"
```

---

## Task 7: Create test fixture cdylib plugin

**Files:**
- Create: `tests/fixtures/example-plugin/Cargo.toml`
- Create: `tests/fixtures/example-plugin/src/lib.rs`

- [ ] **Step 1: Create fixture Cargo.toml**

Create `tests/fixtures/example-plugin/Cargo.toml`:

```toml
[package]
name = "example-plugin"
version = "0.1.0"
edition = "2021"

[lib]
name = "example"
crate-type = ["cdylib"]
```

- [ ] **Step 2: Create fixture lib.rs implementing the 14 ABI symbols**

Create `tests/fixtures/example-plugin/src/lib.rs`:

```rust
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
    HANDLE = 1 as *mut c_void;
    HANDLE
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
    HANDLE = ptr::null_mut();
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
```

- [ ] **Step 3: Build the fixture**

Run: `cd tests/fixtures/example-plugin && cargo build --release`

Expected: Compiles to `target/release/libexample.so` (linux).

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/example-plugin/
git commit -m "test(plugin): add example-plugin cdylib fixture"
```

---

## Task 8: Extend server.js with plugin routes

**Files:**
- Modify: `server.js`

- [ ] **Step 1: Read current server.js**

Run: `head -50 server.js` to understand the existing structure.

- [ ] **Step 2: Add plugin routes to server.js**

Add these routes to `server.js` (insert before the `server.listen` call):

```javascript
// ── Plugin distribution routes ─────────────────────────────────────
// GET /plugins/<name>/manifest.json
// GET /plugins/<name>/<version>/<platform>/<filename>

const PLUGIN_DIST = path.join(__dirname, 'dist', 'plugins');

app.get('/plugins/:name/manifest.json', (req, res) => {
  const manifestPath = path.join(PLUGIN_DIST, req.params.name, 'manifest.json');
  res.sendFile(manifestPath, (err) => {
    if (err) {
      res.status(404).json({ error: 'plugin not found' });
    }
  });
});

app.get('/plugins/:name/:version/:platform/:filename', (req, res) => {
  const { name, version, platform, filename } = req.params;
  // Prevent path traversal.
  if (filename.includes('..') || version.includes('..') || platform.includes('..')) {
    return res.status(400).json({ error: 'invalid path' });
  }
  const filePath = path.join(PLUGIN_DIST, name, version, platform, filename);
  res.sendFile(filePath, (err) => {
    if (err) {
      res.status(404).json({ error: 'file not found' });
    }
  });
});
```

- [ ] **Step 3: Create dist/plugins directory structure for the example plugin**

Run: `mkdir -p dist/plugins/example/0.1.0/linux-x86_64`

Copy the built fixture:
```
cp tests/fixtures/example-plugin/target/release/libexample.so dist/plugins/example/0.1.0/linux-x86_64/
```

- [ ] **Step 4: Create manifest.json for the example plugin**

Create `dist/plugins/example/manifest.json`:

```json
{
  "name": "example",
  "version": "0.1.0",
  "abi_version": 1,
  "dyyl_min": "0.2.0",
  "panic_mode": "abort",
  "commands": [
    {"name": "greet", "arity": 1, "brief": "Send a greeting"},
    {"name": "math.double", "arity": 1, "brief": "Double a number"}
  ],
  "platforms": [
    {"platform": "linux-x86_64", "url": "http://localhost:3000/plugins/example/0.1.0/linux-x86_64/libexample.so", "sha256": "PLACEHOLDER"}
  ]
}
```

- [ ] **Step 5: Compute and fill in the SHA256**

Run: `sha256sum dist/plugins/example/0.1.0/linux-x86_64/libexample.so`

Replace `"PLACEHOLDER"` in manifest.json with the actual hash.

- [ ] **Step 6: Commit**

```bash
git add server.js dist/plugins/
git commit -m "feat(server): add plugin distribution routes + example plugin dist"
```

---

## Task 9: Create publish-plugin.sh script

**Files:**
- Create: `scripts/publish-plugin.sh`

- [ ] **Step 1: Create the script**

Create `scripts/publish-plugin.sh`:

```bash
#!/usr/bin/env bash
# Pack a plugin, compute SHA256, and generate manifest.json.
#
# Usage: ./scripts/publish-plugin.sh <plugin_name> <version> <lib_path>
#
# Outputs to dist/plugins/<name>/manifest.json and
# dist/plugins/<name>/<version>/<platform>/<filename>.

set -euo pipefail

if [ $# -ne 3 ]; then
  echo "Usage: $0 <plugin_name> <version> <lib_path>" >&2
  exit 1
fi

NAME="$1"
VERSION="$2"
LIB_PATH="$3"

# Detect platform.
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
PLATFORM="${OS}-${ARCH}"

# Determine filename.
FILENAME="$(basename "$LIB_PATH")"

# Output directory.
OUT_DIR="dist/plugins/${NAME}/${VERSION}/${PLATFORM}"
mkdir -p "$OUT_DIR"

# Copy library.
cp "$LIB_PATH" "$OUT_DIR/$FILENAME"

# Compute SHA256.
SHA256="$(sha256sum "$OUT_DIR/$FILENAME" | cut -d' ' -f1)"

# Generate manifest.json (merge if exists).
MANIFEST="dist/plugins/${NAME}/manifest.json"
mkdir -p "$(dirname "$MANIFEST")"

# Build URL (assumes l.dyyapp.com or localhost for dev).
BASE_URL="${PLUGIN_BASE_URL:-https://l.dyyapp.com}"
URL="${BASE_URL}/plugins/${NAME}/${VERSION}/${PLATFORM}/${FILENAME}"

cat > "$MANIFEST" <<EOF
{
  "name": "${NAME}",
  "version": "${VERSION}",
  "abi_version": 1,
  "dyyl_min": "0.2.0",
  "panic_mode": "abort",
  "commands": [],
  "platforms": [
    {"platform": "${PLATFORM}", "url": "${URL}", "sha256": "${SHA256}"}
  ]
}
EOF

echo "Published ${NAME} ${VERSION} to ${OUT_DIR}"
echo "Manifest: ${MANIFEST}"
echo "SHA256: ${SHA256}"
```

- [ ] **Step 2: Make executable**

Run: `chmod +x scripts/publish-plugin.sh`

- [ ] **Step 3: Commit**

```bash
git add scripts/publish-plugin.sh
git commit -m "feat(scripts): add publish-plugin.sh helper"
```

---

## Task 10: End-to-end integration test

**Files:**
- Create: `tests/plugin_e2e_tests.rs`

- [ ] **Step 1: Write E2E test that loads the fixture plugin directly**

Create `tests/plugin_e2e_tests.rs`:

```rust
//! End-to-end plugin tests — load the example fixture directly (bypassing
//! fetch) and verify dispatch works.

use dyyl::runtime::plugin::loader::PluginLoader;
use std::path::PathBuf;

fn fixture_lib_path() -> PathBuf {
    // The fixture is built as a cdylib in tests/fixtures/example-plugin/.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/example-plugin/target/release")
        .join(if cfg!(target_os = "macos") {
            "libexample.dylib"
        } else if cfg!(target_os = "windows") {
            "example.dll"
        } else {
            "libexample.so"
        })
}

#[test]
fn load_and_call_greet() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example").expect("load failed");

    // List commands.
    let cmds = loader.list_commands().expect("list_commands failed");
    assert!(cmds.contains("greet"));
    assert!(cmds.contains("math.double"));

    // Call greet.
    let args = r#"[{"type":"str","value":"World"}]"#;
    let result = loader.handle_command("greet", args).expect("greet failed");
    assert!(result.contains("Hello, World!"));
}

#[test]
fn load_and_call_math_double() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example").expect("load failed");

    let args = r#"[{"type":"num","value":"21"}]"#;
    let result = loader.handle_command("math.double", args).expect("math.double failed");
    assert!(result.contains("42"));
}

#[test]
fn load_and_call_unknown_command() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example").expect("load failed");

    let args = "[]";
    let result = loader.handle_command("nonexistent", args);
    assert!(result.is_err(), "unknown command should fail");
}
```

- [ ] **Step 2: Build the fixture**

Run: `cd tests/fixtures/example-plugin && cargo build --release && cd /workspace`

- [ ] **Step 3: Run the E2E test**

Run: `cargo test --test plugin_e2e_tests`

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/plugin_e2e_tests.rs
git commit -m "test(plugin): add end-to-end integration tests with fixture"
```

---

## Task 11: Final verification

**Files:** none

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: All tests pass (except pre-existing unrelated config test).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets --all-features 2>&1 | grep "^error" | head`

Expected: No new errors.

- [ ] **Step 3: Verify CLI commands work**

Run:
```
cargo run -- list
cargo run -- install 2>&1 | head -2
cargo run -- remove 2>&1 | head -2
cargo run -- --lang zh list
```

Expected: Each prints appropriate message (in Chinese for `--lang zh`).

- [ ] **Step 4: Verify server.js serves plugin routes**

Start server: `node server.js &`

Run:
```
curl http://localhost:3000/plugins/example/manifest.json | head
curl -sI http://localhost:3000/plugins/example/0.1.0/linux-x86_64/libexample.so | head -1
```

Expected: manifest.json returned, library file served with 200.

Kill server.

- [ ] **Step 5: fmt + commit if needed**

```bash
cargo fmt
git add -A
git commit -m "style: fmt" || echo "nothing to format"
```

---

## Self-Review Checklist

- [ ] `src/cli/mod.rs`: subcommand detection works, `--lang` parsed before subcommand
- [ ] `plugin_cmds.rs`: all 5 subcommands implemented (install/update/remove/autoremove/list)
- [ ] `main.rs`: subcommand check happens before normal arg parsing
- [ ] `server.js`: `/plugins/<name>/manifest.json` and `/plugins/<name>/<version>/<platform>/<filename>` routes added
- [ ] `publish-plugin.sh`: packs + hashes + generates manifest
- [ ] `tests/fixtures/example-plugin/`: cdylib builds, implements 14 ABI symbols
- [ ] E2E tests: greet, math.double, unknown command all work
- [ ] CLI `--lang zh` produces Chinese output
- [ ] All tests pass (except pre-existing unrelated failure)
- [ ] clippy clean on new code
