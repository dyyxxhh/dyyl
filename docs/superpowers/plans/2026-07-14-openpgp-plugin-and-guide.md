# OpenPGP Plugin and Plugin Development Guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a production OpenPGP plugin (`plugins/openpgp/`) using `sequoia-openpgp`, extend dyyl's credentials system with `file`/`directory` field types, and write a comprehensive plugin development guide (`docs/plugin-development-guide.md`).

**Architecture:** Two-phase: (1) extend dyyl core credentials system to support large/dynamic blobs via `type:"file"` and `type:"directory"` manifest fields, wiring the existing-but-unused `credentials_json` path through `PluginManager`; (2) build the OpenPGP plugin cdylib in a new `plugins/openpgp/` crate with sequoia-based commands plus an independent `gpg.*` family that shells out to system gpg. The guide is written last so it can reference real, tested code.

**Tech Stack:** Rust 2021, `sequoia-openpgp` 2.x, `serde`/`serde_json`, `anyhow`, `chrono`, `shell-words`, `which`, `base64`, dyyl's existing `libloading`/`reqwest`/`toml`/`directories` stack.

**Spec:** [docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)

---

## File Structure

### dyyl core (credentials extension + wiring)

- **Create:** `src/runtime/plugin/creds_inject.rs` — `build_credentials_json()` resolves string/file/directory types.
- **Modify:** `src/runtime/plugin/mod.rs` — wire `assemble_credentials` into `load_plugin`, replace `None` at line ~161 with credentials JSON.
- **Modify:** `src/credentials.rs` — add `credentials_dir_for_plugin()` + `ensure_plugin_credentials()` (interactive prompt).
- **Modify:** `locales/en.json` + `locales/zh.json` — new i18n keys.
- **Create:** `tests/plugin_credentials_inject_tests.rs` — unit tests for `build_credentials_json`.
- **Modify:** `tests/credentials_tests.rs` — add file/directory type cases.

### OpenPGP plugin crate (`plugins/openpgp/`)

- `Cargo.toml`, `plugin.toml.in`, `command_list.json`, `.gitignore`
- `src/lib.rs` (15 ABI symbols + dispatch), `src/state.rs`, `src/codec.rs`, `src/error.rs`, `src/creds.rs`, `src/keyring.rs`
- `src/commands/mod.rs` + `key.rs`, `encrypt.rs`, `decrypt.rs`, `sign.rs`, `verify.rs`, `armor.rs`, `gpg.rs`
- `tests/keyring_tests.rs`, `key_tests.rs`, `encrypt_decrypt_tests.rs`, `sign_verify_tests.rs`, `armor_tests.rs`, `gpg_tests.rs`

### dyyl integration

- `tests/fixtures/build-openpgp.sh`, `tests/openpgp_plugin_tests.rs`, `tests/openpgp_e2e_tests.rs`, `tests/openpgp_gpg_tests.rs`
- `tests/fixtures/openpgp-*.dyyl` golden scripts

### Docs/publish

- `scripts/publish-plugin.sh` (modify), `docs/plugin-development-guide.md` (create), `README.md` + `dyyl-api-reference.md` (modify)

---

## Task 1: Add `credentials_dir_for_plugin` helper

**Files:** Modify `src/credentials.rs`, `src/lib.rs`; Test `tests/credentials_tests.rs`

- [ ] **Step 1: Write failing test** in `tests/credentials_tests.rs`:

```rust
#[test]
fn credentials_dir_for_plugin_returns_xdg_data_path() {
    std::env::set_var("XDG_DATA_HOME", "/tmp/dyyl-test-xdg");
    let dir = dyyl::credentials::credentials_dir_for_plugin("openpgp");
    assert!(dir.ends_with("dyyl/credentials.d/openpgp"));
    std::env::remove_var("XDG_DATA_HOME");
}
```

- [ ] **Step 2: Run to verify failure**

`cargo test --test credentials_tests credentials_dir_for_plugin_returns_xdg_data_path` → FAIL (function not found).

- [ ] **Step 3: Add helper to `src/credentials.rs`** (append):

```rust
/// `<xdg_data>/dyyl/credentials.d/<plugin_name>/`. Does NOT create the dir.
#[must_use]
pub fn credentials_dir_for_plugin(plugin_name: &str) -> std::path::PathBuf {
    let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")
        .expect("unable to determine XDG data directory");
    proj.data_dir().join("credentials.d").join(plugin_name)
}
```

Verify `src/lib.rs` has `pub mod credentials;` (add if missing).

- [ ] **Step 4: Run to verify pass** → `cargo test --test credentials_tests credentials_dir_for_plugin_returns_xdg_data_path`
- [ ] **Step 5: Commit** → `git add src/credentials.rs src/lib.rs tests/credentials_tests.rs && git commit -m "feat(credentials): add credentials_dir_for_plugin helper"`

---

## Task 2: Create `creds_inject.rs` — `build_credentials_json`

**Files:** Create `src/runtime/plugin/creds_inject.rs`; Modify `src/runtime/plugin/mod.rs`; Test `tests/plugin_credentials_inject_tests.rs`

- [ ] **Step 1: Create `src/runtime/plugin/creds_inject.rs`**:

```rust
//! Build the credentials JSON passed to `dyyl_plugin_set_credentials`.
//! Resolves string/file/directory field types against credentials.toml + filesystem.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::credentials::credentials_dir_for_plugin;
use crate::i18n::Lang;
use crate::runtime::plugin::manifest::{CredentialField, CredentialsSpec};

pub fn build_credentials_json(
    spec: Option<&CredentialsSpec>,
    plugin_name: &str,
    toml_fields: &HashMap<String, String>,
    _lang: Lang,
) -> Result<String, String> {
    let creds_dir = credentials_dir_for_plugin(plugin_name);
    let mut map: HashMap<String, String> = HashMap::new();
    ensure_credentials_dir(&creds_dir)?;
    map.insert("__credentials_dir".to_string(), creds_dir.to_string_lossy().to_string());

    if let Some(spec) = spec {
        for field in &spec.fields {
            map.insert(field.name.clone(), resolve_field(field, plugin_name, &creds_dir, toml_fields)?);
        }
    }
    serde_json::to_string(&map).map_err(|e| format!("serialize credentials json: {e}"))
}

fn resolve_field(field: &CredentialField, plugin_name: &str, creds_dir: &PathBuf, toml_fields: &HashMap<String, String>) -> Result<String, String> {
    match field.r#type.as_str() {
        "string" => toml_fields.get(&field.name).cloned()
            .ok_or_else(|| format!("missing string credential '{}' for plugin '{}'", field.name, plugin_name)),
        "file" => {
            let path = creds_dir.join(&field.name);
            if path.exists() {
                fs::read_to_string(&path).map_err(|e| format!("read credential file {}: {e}", path.display()))
            } else {
                eprintln!("warning: credential file '{}' for plugin '{}' not found, injecting empty", path.display(), plugin_name);
                Ok(String::new())
            }
        }
        "directory" => Ok(creds_dir.join(&field.name).to_string_lossy().to_string()),
        other => Err(format!("unknown credential field type '{other}' for field '{}'", field.name)),
    }
}

fn ensure_credentials_dir(dir: &PathBuf) -> Result<(), String> {
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("create credentials dir {}: {e}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Register module** in `src/runtime/plugin/mod.rs` near other `pub mod` lines: `pub mod creds_inject;`

- [ ] **Step 3: Write failing tests** in `tests/plugin_credentials_inject_tests.rs` — test string/file/directory types, missing file → empty, auto-creation with 0700, `__credentials_dir` auto-injection. See spec §5 for behavior. Use `with_temp_xdg` helper that sets `XDG_DATA_HOME` to a tempdir.

- [ ] **Step 4: Run to verify failure** → `cargo test --test plugin_credentials_inject_tests` → FAIL (visibility)
- [ ] **Step 5: Fix visibility** — ensure `runtime::plugin::creds_inject`, `runtime::plugin::manifest::{CredentialField, CredentialsSpec}`, `i18n::Lang` are `pub` and reachable from the test crate. Add `pub use` re-exports in `src/lib.rs` as needed.
- [ ] **Step 6: Run to verify pass** → `cargo test --test plugin_credentials_inject_tests` → PASS (all tests)
- [ ] **Step 7: Clippy** → `cargo clippy --all-targets --all-features` → no warnings
- [ ] **Step 8: Commit** → `git add src/runtime/plugin/creds_inject.rs src/runtime/plugin/mod.rs src/lib.rs tests/plugin_credentials_inject_tests.rs && git commit -m "feat(plugin): add creds_inject module for file/directory credential types"`

---

## Task 3: Wire credentials through `PluginManager.load_plugin`

**Files:** Modify `src/runtime/plugin/mod.rs`, `locales/en.json`, `locales/zh.json`

Currently at [mod.rs:161](file:///workspace/src/runtime/plugin/mod.rs#L161) the call passes `None`. Replace with credentials assembly.

- [ ] **Step 1: Add i18n keys** to both locale files:
  - en: `"plugin.credentials_missing": "plugin '{name}' missing credentials: {fields}"`
  - zh: `"plugin.credentials_missing": "插件 '{name}' 缺少凭证: {fields}"`

- [ ] **Step 2: Add `assemble_credentials` method** to `PluginManager` in `src/runtime/plugin/mod.rs`. It reads `credentials.toml`, checks for missing `string` fields (triggers `ensure_plugin_credentials` if missing — see Task 4), then calls `build_credentials_json`. Replace `None` at line 161 with `Some(&credentials_json)`. See spec §3.2 for the JSON shape.

- [ ] **Step 3: Run existing plugin tests** for regressions → `cargo test --test plugin_e2e_tests --test plugin_credentials_tests --test plugin_registry_tests` → PASS
- [ ] **Step 4: Clippy** → no warnings
- [ ] **Step 5: Commit** → `git commit -m "feat(plugin): wire credentials JSON through PluginManager.load_plugin"`

---

## Task 4: Interactive prompt for missing plugin credentials

**Files:** Modify `src/credentials.rs`, `locales/en.json`, `locales/zh.json`; Test `tests/credentials_tests.rs`

- [ ] **Step 1: Add `ensure_plugin_credentials` to `src/credentials.rs`** — loads `credentials.toml`, finds missing `string` fields, prompts via stderr+stdin, saves. See spec §5.6. Returns `Err` on EOF (CI behavior).

- [ ] **Step 2: Add i18n keys**:
  - en: `"plugin.credential_prompt_header": "[dyyl] plugin '{name}' needs credentials, please enter:"`, `"plugin.credential_saved": "[dyyl] credentials saved to {path}"`
  - zh: corresponding translations

- [ ] **Step 3: Add EOF test** to `tests/credentials_tests.rs` — verifies the function returns `Err` when stdin is closed.

- [ ] **Step 4: Run tests** → `cargo test --test credentials_tests ensure_plugin_credentials` → PASS
- [ ] **Step 5: Full regression** → `cargo test` → all existing tests pass
- [ ] **Step 6: Commit** → `git commit -m "feat(credentials): interactive prompt for missing plugin credentials"`

---

## Task 5: Scaffold the OpenPGP plugin crate

**Files:** Create `plugins/openpgp/Cargo.toml`, `.gitignore`, `plugin.toml.in`, `command_list.json`, `src/lib.rs`, `src/state.rs`, `src/codec.rs`, `src/error.rs`, `src/creds.rs`, `src/keyring.rs` (stub), `src/commands/mod.rs` + 7 stub command modules

- [ ] **Step 1: Create `plugins/openpgp/Cargo.toml`** with cdylib, `panic = "abort"`, deps: `sequoia-openpgp` (v2, default-features=false, features=["compression-deflate"]), `serde`, `serde_json`, `anyhow`, `chrono`, `shell-words`, `which`, `base64`. If `default-features=false` breaks the crypto backend at build time, switch to `default-features=true`.

- [ ] **Step 2: Create `plugins/openpgp/.gitignore`** → `target/`

- [ ] **Step 3: Create `plugins/openpgp/src/state.rs`** — `PluginState` struct with `default_passphrase`, `default_key`, `credentials_dir`, `key_cache: Mutex<HashMap<String, String>>`, `index: Mutex<Option<KeyringIndex>>`. Plus `KeyringIndex` and `KeyringEntry` structs (serde Serialize/Deserialize). `clear_cache()` method.

- [ ] **Step 4: Create `plugins/openpgp/src/codec.rs`** — `DyylValue` enum (Num/Str/Empty/List/Dict), `decode_args(json) -> Result<Vec<DyylValue>>`, `encode_out(out, v)`. Symmetric with dyyl's [value_codec.rs](file:///workspace/src/runtime/plugin/value_codec.rs). See spec §6.4 for exact encoding.

- [ ] **Step 5: Create `plugins/openpgp/src/error.rs`** — `PluginError` struct with `code`/`message`, `write_error()` helper, convenience constructors for all 10 error codes (arity_mismatch, type_error, unknown_command, runtime, key_not_found, passphrase_wrong, parse_failed, verify_failed, gpg_not_installed, gpg_exec_failed). See spec §6.5.

- [ ] **Step 6: Create `plugins/openpgp/src/creds.rs`** — `apply_credentials(state, json)` parses JSON into `PluginState` fields (passphrase, default_key, credentials_dir).

- [ ] **Step 7: Create `plugins/openpgp/src/keyring.rs`** stub (1-line comment, implemented in Task 6).

- [ ] **Step 8: Create `plugins/openpgp/src/commands/mod.rs`** — `dispatch(state, cmd, args)` match with all 30 command routes. See spec §4.1/§4.2 for the full list. `pub mod` declarations for all 7 submodules.

- [ ] **Step 9: Create 7 stub command modules** (`key.rs`, `encrypt.rs`, `decrypt.rs`, `sign.rs`, `verify.rs`, `armor.rs`, `gpg.rs`) in `plugins/openpgp/src/commands/`. Each function returns `Err(PluginError::runtime("not yet implemented"))`. Function signatures: `pub fn <name>(_state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError>`. Functions per module listed in spec §6.1.

- [ ] **Step 10: Create `plugins/openpgp/command_list.json`** — JSON array of 30 `{name, arity, brief}` objects matching spec §4.1/§4.2. This file is `include_str!`'d by `lib.rs`.

- [ ] **Step 11: Create `plugins/openpgp/plugin.toml.in`** — TOML manifest with all 30 `[[commands]]` entries + `[[credentials.fields]]` for passphrase/default_key/__credentials_dir. See spec §7.2.

- [ ] **Step 12: Create `plugins/openpgp/src/lib.rs`** — 15 `#[no_mangle] extern "C"` ABI symbols. `handle_command` calls `commands::dispatch`. `list_commands` returns `include_str!("../command_list.json")`. `set_credentials` calls `creds::apply_credentials`. `init` allocates `Box::new(PluginState::default())` and returns raw pointer. `shutdown` does `Box::from_raw`. See spec §6.3.

- [ ] **Step 13: Build the plugin** → `cd plugins/openpgp && cargo build --release` → produces `target/release/libopenpgp.so`. Fix any sequoia backend issues by toggling `default-features`.

- [ ] **Step 14: Commit** → `git add plugins/openpgp/ && git commit -m "feat(openpgp): scaffold plugin crate with 15 ABI symbols + 30 command stubs"`

---

## Task 6: Implement `keyring.rs`

**Files:** Modify `plugins/openpgp/src/keyring.rs`; Test `plugins/openpgp/tests/keyring_tests.rs`

- [ ] **Step 1: Write failing tests** in `plugins/openpgp/tests/keyring_tests.rs` — test `load_index` (empty + roundtrip), `upsert_entry` (insert + merge by fp), `remove_entry` (deletes index + files), `read_key_file`/`write_key_file` roundtrip. Use tempdir for `base_dir`.

- [ ] **Step 2: Run to verify failure** → `cd plugins/openpgp && cargo test --test keyring_tests` → FAIL
- [ ] **Step 3: Implement `Keyring` struct** with `base_dir: PathBuf`, methods: `load_index`, `save_index`, `upsert_entry`, `remove_entry`, `write_key_file` (0600 on sec), `read_key_file`, `find_entry`. Paths: `keys/<fp>.{pub,sec}.asc`, `index.json`. See spec §5.4.
- [ ] **Step 4: Run to verify pass** → all 5 tests PASS
- [ ] **Step 5: Clippy** → no warnings
- [ ] **Step 6: Commit** → `git commit -m "feat(openpgp): implement keyring CRUD with index.json"`

---

## Task 7: Implement `commands/key.rs`

**Files:** Modify `plugins/openpgp/src/commands/key.rs`; Test `plugins/openpgp/tests/key_tests.rs`

- [ ] **Step 1: Write failing tests** — `key.generate` returns fingerprint + writes files + updates index; `key.generate` with custom passphrase; `key.list` empty/non-empty; `key.delete` removes; `key.delete` nonexistent → error. Use helper that creates a temp state with `credentials_dir` set.

- [ ] **Step 2: Run to verify failure** → FAIL
- [ ] **Step 3: Implement** `generate` (CertBuilder with Ed25519+Curve25519 subkeys, passphrase, armor serialize pub+sec, write to keyring), `import` (parse armored, detect pub/sec, re-serialize, write), `export` (read file), `list` (load index, build list of dicts), `delete` (find_entry check + remove_entry). Use `sequoia_openpgp::cert::prelude::*`. See spec §4.1 signatures.
- [ ] **Step 4: Run to verify pass** → all 5 tests PASS
- [ ] **Step 5: Clippy** → no warnings
- [ ] **Step 6: Commit** → `git commit -m "feat(openpgp): implement key.generate/import/export/list/delete"`

---

## Task 8: Implement `commands/encrypt.rs` and `commands/decrypt.rs`

**Files:** Modify both; Test `plugins/openpgp/tests/encrypt_decrypt_tests.rs`

- [ ] **Step 1: Write failing round-trip tests** — encrypt+decrypt with fp; encrypt with inline armored pubkey; encrypt nonexistent fp → error; sym.encrypt+sym.decrypt; decrypt wrong passphrase → error.

- [ ] **Step 2: Run to verify failure** → FAIL
- [ ] **Step 3: Implement `encrypt.rs`** — `encrypt` (resolve recipients: fp lookup or inline armored, Encryptor2::for_recipients, LiteralWriter, armor output), `encrypt_file` (read file, call encrypt, write output), `sym_encrypt` (Encryptor2::with_passphrases). See spec §4.1.
- [ ] **Step 4: Implement `decrypt.rs`** — `decrypt` (dearmor, DecryptorBuilder with passphrase, read plaintext), `decrypt_file`, `sym_decrypt`. `resolve_passphrase` helper: arg override > default > error. See spec §4.1.
- [ ] **Step 5: Run to verify pass** → all 5 tests PASS (adjust sequoia 2.x API signatures as needed)
- [ ] **Step 6: Clippy** → no warnings
- [ ] **Step 7: Commit** → `git commit -m "feat(openpgp): implement encrypt/decrypt + symmetric variants"`

---

## Task 9: Implement `commands/sign.rs` and `commands/verify.rs`

**Files:** Modify both; Test `plugins/openpgp/tests/sign_verify_tests.rs`

- [ ] **Step 1: Write failing tests** — sign+verify inline roundtrip; sign detached+verify detached; sign wrong passphrase → error; verify tampered → `{valid:"0"}`.

- [ ] **Step 2: Run to verify failure** → FAIL
- [ ] **Step 3: Implement `sign.rs`** — `load_signer` (read sec key, decrypt with passphrase, into_keypair), `sign` (inline: Signer+LiteralWriter; detached: Armorer+Signer::detached), `sign_file`. See spec §4.1.
- [ ] **Step 4: Implement `verify.rs`** — `VerifyHelper` impl (loads all pub certs from keyring, tracks `found_valid`), `verify` (inline: VerifierBuilder; detached: VerifierBuilder with data), `verify_file`. Returns dict `{valid, signer_uid, signer_fp}`. See spec §4.1. Note: sequoia's streaming verifier API may differ for detached vs inline — consult sequoia 2.x docs.
- [ ] **Step 5: Run to verify pass** → all 4 tests PASS (adjust sequoia API as needed)
- [ ] **Step 6: Clippy** → no warnings
- [ ] **Step 7: Commit** → `git commit -m "feat(openpgp): implement sign/verify (inline + detached)"`

---

## Task 10: Implement `commands/armor.rs` and `commands/gpg.rs`

**Files:** Modify both; Tests `plugins/openpgp/tests/armor_tests.rs`, `plugins/openpgp/tests/gpg_tests.rs`

- [ ] **Step 1: Write armor test** — `armor`+`dearmor` roundtrip with base64 input.
- [ ] **Step 2: Write gpg tests** — `gpg.detect` (skip if gpg not installed), `gpg.exec "--version"`, `gpg.encrypt`+`gpg.decrypt` roundtrip (generate test key in isolated GNUPGHOME first). All gpg tests guard with `gpg_available()` check.

- [ ] **Step 3: Run to verify failure** → FAIL
- [ ] **Step 4: Implement `armor.rs`** — `armor` (base64 decode → sequoia armor Writer Kind::File), `dearmor` (armor Reader → base64 encode). Uses `base64` crate.
- [ ] **Step 5: Implement `gpg.rs`** — `gpg_path()` via `which`, `run_gpg(args, stdin)` helper (spawn, write stdin, check exit code, return stdout or gpg_exec_failed error), `detect` (returns `{installed, path, version}`), `exec` (shell-words split or list, optional stdin), `encrypt`/`encrypt_file`/`decrypt`/`decrypt_file`/`sign`/`sign_file`/`verify`/`verify_file`/`key_list`/`key_import`/`key_export`. See spec §4.2 for each command's gpg args. `verify` returns `{valid, signer}` dict. None of these read `PluginState` credentials/keyring (independent family).
- [ ] **Step 6: Run to verify pass** → armor PASS, gpg tests PASS (or skip if gpg absent)
- [ ] **Step 7: Clippy** → no warnings
- [ ] **Step 8: Commit** → `git commit -m "feat(openpgp): implement armor/dearmor + full gpg.* command family"`

---

## Task 11: dyyl integration tests (real dlopen)

**Files:** Create `tests/fixtures/build-openpgp.sh`, `tests/openpgp_plugin_tests.rs`

- [ ] **Step 1: Create `tests/fixtures/build-openpgp.sh`** — runs `cd plugins/openpgp && cargo build --release`, copies `libopenpgp.so` (or `.dylib`) to a tempdir passed as arg. `chmod +x`.

- [ ] **Step 2: Write integration test** `tests/openpgp_plugin_tests.rs` — uses `libloading` directly (not dyyl's PluginLoader) to test the raw ABI. Tests: (a) load library + resolve all 15 symbols, `get_api_version()` returns 2; (b) `init(2)` → non-null handle, `set_credentials` with test JSON, `on_load`, `handle_command("key.generate", [...])` returns success with a string fingerprint. Set `XDG_DATA_HOME` to tempdir for isolation.

- [ ] **Step 3: Run** → `cargo test --test openpgp_plugin_tests` → PASS (first run builds the plugin, may be slow)
- [ ] **Step 4: Commit** → `git commit -m "test(openpgp): add dlopen integration tests"`

---

## Task 12: e2e golden scripts

**Files:** Create 5 `.dyyl` fixtures + `tests/openpgp_e2e_tests.rs`

- [ ] **Step 1: Create golden scripts** in `tests/fixtures/`:
  - `openpgp-roundtrip.dyyl`: generate → encrypt → decrypt → assert
  - `openpgp-sign-verify.dyyl`: generate → sign detached → verify → assert valid:"1"
  - `openpgp-sym.dyyl`: sym.encrypt → sym.decrypt → assert
  - `openpgp-gpg-detect.dyyl`: gpg.detect → print installed field
  - `openpgp-keyring-persist.dyyl`: generate → key.list → assert len:"1"

- [ ] **Step 2: Write e2e runner** `tests/openpgp_e2e_tests.rs` — builds the plugin via `build-openpgp.sh`, installs it to a temp plugins dir (sets `XDG_DATA_HOME`), runs `target/release/dyyl <script>.dyyl` for each fixture, asserts exit code 0 and expected output. gpg-detect script is allowed to print "0" or "1".

- [ ] **Step 3: Run** → `cargo test --test openpgp_e2e_tests` → PASS
- [ ] **Step 4: Commit** → `git commit -m "test(openpgp): add e2e golden scripts"`

---

## Task 13: Extend `scripts/publish-plugin.sh`

**Files:** Modify `scripts/publish-plugin.sh`

- [ ] **Step 1: Read existing script** to understand current structure.
- [ ] **Step 2: Extend to accept source dir arg** — `./scripts/publish-plugin.sh plugins/openpgp`. Read `plugin.toml.in`, build the cdylib (`cargo build --release` in the source dir), copy to `dist/plugins/<name>/<version>/<platform>/`, compute SHA256, generate `manifest.json` with url based on `DYRL_DIST_HOST` env var (default `http://localhost:8951`). Support `--target` for cross-platform.
- [ ] **Step 3: Test manually** → `./scripts/publish-plugin.sh plugins/openpgp` → produces `dist/plugins/openpgp/manifest.json` + `.so` file
- [ ] **Step 4: Verify server.js serves it** → start `node server.js`, `curl http://localhost:8951/plugins/openpgp/manifest.json` returns JSON
- [ ] **Step 5: Commit** → `git commit -m "feat(publish): extend publish-plugin.sh for source-dir plugins"`

---

## Task 14: Write `docs/plugin-development-guide.md`

**Files:** Create `docs/plugin-development-guide.md`

The 14-chapter guide per spec §9.2. Written in Chinese, code comments in Chinese. OpenPGP plugin is the running example in Chapter 11. Uses `file:///` links to reference real code.

- [ ] **Step 1: Write Chapter 1 (简介)** — what is a dyyl plugin, when to write one, UB risk preview.
- [ ] **Step 2: Write Chapter 2 (快速开始)** — minimal 30-line plugin, cdylib setup, 15 symbols, build, server.js distribution, script call. Reference [tests/fixtures/example-plugin/](file:///workspace/tests/fixtures/example-plugin/).
- [ ] **Step 3: Write Chapter 3 (架构与生命周期)** — dispatch flow, load/unload timing, handle ownership, panic=abort rationale.
- [ ] **Step 4: Write Chapter 4 (C ABI 契约)** — 15 symbol table with signatures, export template code, string memory conventions, ABI v1/v2. Reference [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs).
- [ ] **Step 5: Write Chapter 5 (Value JSON 编解码)** — 6 Value types, args array format, out single-value format, num-as-string rationale, DyylValue Rust enum. Reference [src/runtime/plugin/value_codec.rs](file:///workspace/src/runtime/plugin/value_codec.rs) and [plugins/openpgp/src/codec.rs](file:///workspace/plugins/openpgp/src/codec.rs).
- [ ] **Step 6: Write Chapter 6 (Manifest 与 plugin.toml)** — manifest.json schema, plugin.toml, multi-level command names, platforms, abi_version/dyyl_min/panic_mode, has_locales.
- [ ] **Step 7: Write Chapter 7 (Credentials 系统)** — credentials.toml structure, manifest fields, three types (string/file/directory), `__credentials_dir` auto-injection, set_credentials timing, interactive prompt, permissions, large/dynamic credential pattern with OpenPGP keyring as example. Reference [src/runtime/plugin/creds_inject.rs](file:///workspace/src/runtime/plugin/creds_inject.rs) and [plugins/openpgp/src/keyring.rs](file:///workspace/plugins/openpgp/src/keyring.rs).
- [ ] **Step 8: Write Chapter 8 (i18n)** — locales/en.json + zh.json, has_locales, register_plugin, key naming `<plugin>.<key>`, zh→en fallback. Reference [src/i18n.rs](file:///workspace/src/i18n.rs).
- [ ] **Step 9: Write Chapter 9 (构建、发布与分发)** — Cargo.toml cdylib config, single/cross-platform build, publish-plugin.sh, server.js, SHA256, version/ABI strategy. Reference [plugins/openpgp/Cargo.toml](file:///workspace/plugins/openpgp/Cargo.toml) and [scripts/publish-plugin.sh](file:///workspace/scripts/publish-plugin.sh).
- [ ] **Step 10: Write Chapter 10 (测试插件)** — crate unit tests, dyyl integration (dlopen fixture), e2e golden, CI, clippy deny rules.
- [ ] **Step 11: Write Chapter 11 (完整范例：OpenPGP 插件)** — design goals, crate structure, PluginState, handle_command dispatch, key.generate walkthrough, encrypt/decrypt, sign/verify, gpg.* family, credentials.d keyring, error codes, test suite. This is the longest chapter; reference real code with `file:///` links.
- [ ] **Step 12: Write Chapter 12 (已知风险与约束)** — trust model (无限信任), panic=abort hard constraint, credentials.toml plaintext, private key memory best practice, gpg.* is system call wrapper.
- [ ] **Step 13: Write Chapter 13 (故障排查)** — error codes table, dlopen failures, SHA256 mismatch, credentials prompt loop, --debug output, bug reporting.
- [ ] **Step 14: Write Chapter 14 (参考资源)** — links to README, dyyl-api-reference.md, specs, sequoia docs, RFC 4880/6637/9580.
- [ ] **Step 15: Review guide** — check all `file:///` links resolve, code blocks have language tags, no TBD/TODO.
- [ ] **Step 16: Commit** → `git add docs/plugin-development-guide.md && git commit -m "docs: add comprehensive plugin development guide"`

---

## Task 15: Update README.md and dyyl-api-reference.md

**Files:** Modify `README.md`, `dyyl-api-reference.md`

- [ ] **Step 1: Read both files** to understand current structure.
- [ ] **Step 2: Add `openpgp.*` command family to `dyyl-api-reference.md`** — brief entry per command (30 commands), link to [docs/plugin-development-guide.md#11-完整范例openpgp-插件](file:///workspace/docs/plugin-development-guide.md) for details.
- [ ] **Step 3: Add plugin development mention to `README.md`** — short section pointing to the guide, mention OpenPGP as an example plugin.
- [ ] **Step 4: Commit** → `git commit -m "docs: add openpgp command family to API reference + plugin guide link in README"`

---

## Task 16: Final verification

- [ ] **Step 1: Full test suite** → `cargo test` (dyyl core + integration) → all PASS
- [ ] **Step 2: Plugin crate tests** → `cd plugins/openpgp && cargo test` → all PASS
- [ ] **Step 3: Clippy both** → `cargo clippy --all-targets --all-features` + `cd plugins/openpgp && cargo clippy --all-targets` → no warnings
- [ ] **Step 4: Format check** → `cargo fmt --check` → clean
- [ ] **Step 5: Manual smoke test** — install plugin via `dyyl install openpgp`, run `openpgp.key.generate "test" "_"` in a script, verify fingerprint returned
- [ ] **Step 6: Build dist** → `./scripts/publish-plugin.sh plugins/openpgp` → manifest + .so produced
- [ ] **Step 7: Final commit** (if any cleanup needed) → `git commit -m "chore: final verification and cleanup"`
