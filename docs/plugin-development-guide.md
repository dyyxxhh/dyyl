# dyyl 插件开发指南

> 本指南面向会 Rust 的开发者，单文件综合讲解 dyyl 插件系统的全貌：既是教程（第 2、3、11 章从零构建可运行插件），又是参考手册（第 4–8 章给出 ABI 全表、Value codec、manifest schema、credentials、i18n）。OpenPGP 插件是贯穿范例，前面的章节用最小示例，复杂应用在第 11 章逐段展开。
>
> 指南版本对应 dyyl v0.2.0、插件 ABI v2（15 符号）。

---

## 1. 简介

### 1.1 什么是 dyyl 插件

dyyl 插件是一个编译为动态库（Linux `.so`、macOS `.dylib`、Windows `.dll`）的 Rust crate，通过 C ABI 与 dyyl 解释器通信。脚本里直接写 `<plugin_name>.<sub>` 即可调用，无需 `use`/`import` 声明。首次调用时运行时按需下载、校验 SHA256、dlopen 加载，常驻到脚本结束。

完整的生态设计见 [plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)。OpenPGP 插件与扩展的 credentials 类型设计见 [openpgp-plugin-and-guide-design.md](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)。

### 1.2 何时该写插件（vs 用 dyyl 脚本）

写插件适合下列场景：

- 需要调用 Rust 生态的库（如 `sequoia-openpgp`、`reqwest`、`rusqlite`），脚本层无法直接访问。
- 性能敏感的循环或大数据处理（脚本每行一条命令，开销大）。
- 需要跨进程持久化的本地状态（如自管钥匙串目录）。
- 想把现有内置命令族重写为可独立分发的组件（"内置命令迁移到插件"是 dyyl 的长期方向）。

直接用 dyyl 脚本就够的场景：

- 简单的字符串/数值/列表操作（`str.*`、`math.*`、`list.*`、`dict.*` 已经够用）。
- 一次性自动化流程（写完即扔，不需要分发）。
- 不需要外部库依赖的逻辑。

### 1.3 插件能做什么/不能做什么

**能做的：**

- 调用任何 Rust crate（在 `Cargo.toml` 加依赖，编进 cdylib）。
- 读写用户文件系统（与 dyyl 进程同权限，无沙箱）。
- 起子进程、调用系统二进制（OpenPGP 插件的 `gpg.*` 命令族就是包装系统 `gpg`）。
- 在 credentials 目录下持久化状态（如钥匙串、缓存）。
- 注册自己的 i18n 键值表，由 dyyl 在 `t()` 时按语言解析。

**不能做的（UB 风险预告，详见第 12 章）：**

- 跨 FFI 边界 panic：未设 `panic = "abort"` 时跨 FFI panic 是未定义行为（UB），必须把 `panic = "abort"` 写进 `[profile.release]`。dyyl 加载时只做 manifest 字段校验，不强制验证编译选项。
- 自行 `unwrap`/`expect` 跨 FFI 调用结果：插件 crate 应开启 `clippy::unwrap_used`/`clippy::panic` deny（OpenPGP 插件就是这么做的，详见 [Cargo.toml](file:///workspace/plugins/openpgp/Cargo.toml#L29-L38)）。
- 任意 ABI 兼容承诺：dyyl 当前只支持 ABI v1（13 符号 + `free_string`）与 ABI v2（再加 `set_credentials`，共 15 符号）；其它版本号会被拒绝加载（见 [loader.rs](file:///workspace/src/runtime/plugin/loader.rs#L46-L52)）。

---

## 2. 快速开始（30 行最小插件）

本章从零写一个最小可运行的 dyyl 插件 `example`，提供两个命令：`example.greet` 和 `example.math.double`。代码参考 [tests/fixtures/example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs)。

### 2.1 创建 cdylib crate

新建一个空目录，写入 `Cargo.toml`：

```toml
# Cargo.toml —— cdylib crate 配置
[package]
name = "example-plugin"
version = "0.1.0"
edition = "2021"

[lib]
# 产物名是 libexample.so（Linux）/ libexample.dylib（macOS）/ example.dll（Windows）
# lib.name 必须与 manifest 的插件名一致
name = "example"
crate-type = ["cdylib"]

[profile.release]
# 跨 FFI panic 必须 abort（详见 §3.4 与 §12.2）
panic = "abort"
opt-level = 3
```

要点：

- `crate-type = ["cdylib"]` 让 cargo 产出动态库而不是可执行文件或 rlib。
- `lib.name = "example"` 决定产物文件名前缀，dyyl 安装后期望文件名是 `libexample.so` / `libexample.dylib` / `example.dll`（见 [store.rs](file:///workspace/src/runtime/plugin/store.rs#L48-L57)）。
- `panic = "abort"` 在 release profile 必写，否则跨 FFI panic 是 UB。

### 2.2 实现最小 15 符号

ABI v2 共 15 个导出符号，全部 `#[no_mangle] extern "C"`。最小插件的完整源码（参考 [example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs)）：

```rust
// src/lib.rs —— 最小 example 插件

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

// ABI v2 共 15 符号，最小插件用静态哨兵当 handle
static mut HANDLE: *mut c_void = ptr::null_mut();

// 1. API 版本
#[no_mangle]
pub extern "C" fn dyyl_plugin_get_api_version() -> c_uint {
    2
}

// 2-5. 元数据 getter（字符串通过出参返回，插件负责分配）
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

// 6. init —— 返回插件 handle（NULL 表示失败）
#[no_mangle]
pub extern "C" fn dyyl_plugin_init(_api_version: c_uint) -> *mut c_void {
    unsafe {
        HANDLE = 1 as *mut c_void;  // 用静态哨兵当 handle
        HANDLE
    }
}

// 7. on_load —— 加载完成钩子，0=成功
#[no_mangle]
pub extern "C" fn dyyl_plugin_on_load(_handle: *mut c_void) -> c_int {
    0
}

// 8. list_commands —— 输出命令清单 JSON 数组
#[no_mangle]
pub extern "C" fn dyyl_plugin_list_commands(
    _handle: *mut c_void,
    out: *mut *mut c_char,
) -> c_int {
    let json = r#"[{"name":"greet","arity":1,"brief":"Send a greeting"},{"name":"math.double","arity":1,"brief":"Double a number"}]"#;
    write_string(json, out)
}

// 9. get_command_help —— 单命令帮助
#[no_mangle]
pub extern "C" fn dyyl_plugin_get_command_help(
    _handle: *mut c_void,
    _cmd: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    write_string("Help text", out)
}

// 10. handle_command —— 核心调度
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
            let name = extract_first_str_arg(args_str);
            // 输出是单个 Value 的 JSON（详见 §5）
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
            // 失败时 out 写错误对象，返回非 0（详见 §4 错误对象格式）
            let err = r#"{"code":"unknown_command","message":"unknown command"}"#;
            write_string(err, out);
            1
        }
    }
}

// 11. on_error —— dyyl 在调度失败后回调
#[no_mangle]
pub extern "C" fn dyyl_plugin_on_error(
    _handle: *mut c_void,
    _cmd: *const c_char,
    _code: c_int,
    _err: *const c_char,
) -> c_int {
    0
}

// 12. on_unload —— 卸载前钩子
#[no_mangle]
pub extern "C" fn dyyl_plugin_on_unload(_handle: *mut c_void) -> c_int {
    0
}

// 13. shutdown —— 释放 handle
#[no_mangle]
pub extern "C" fn dyyl_plugin_shutdown(_handle: *mut c_void) {
    unsafe { HANDLE = ptr::null_mut(); }
}

// 14. free_string —— 释放插件分配的字符串
#[no_mangle]
pub extern "C" fn dyyl_plugin_free_string(ptr: *mut c_char) {
    unsafe {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);  // 由插件自己 free
        }
    }
}

// 15. set_credentials —— ABI v2 新增，注入凭证 JSON
#[no_mangle]
pub extern "C" fn dyyl_plugin_set_credentials(
    _handle: *mut c_void,
    _creds_json: *const c_char,
) -> c_int {
    0  // 最小插件不读凭证，直接返回 0
}

// ── 辅助函数 ──────────────────────────────────────────────────

fn write_string(s: &str, out: *mut *mut c_char) -> c_int {
    let c = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
    unsafe { *out = c.into_raw(); }  // 通过 CString::into_raw 分配
    0
}

// 最小解析：从 args JSON 数组里抽第一个 str 的 value
fn extract_first_str_arg(args_json: &str) -> String {
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

要点说明：

- `dyyl_plugin_init` 返回的 `*mut c_void` 是插件自己的 handle（任意不透明指针）。最小插件用静态哨兵 `1` 充数；生产级插件应当分配 `Box<State>` 并返回裸指针，详见 §11.3 的 OpenPGP 插件实现。
- 字符串通过 `CString::into_raw` 分配，由 dyyl 调 `dyyl_plugin_free_string` 时再用 `CString::from_raw` 回收。这避免了跨分配器问题（同一进程不同 Rust 版本 allocator 可能不同）。
- `handle_command` 失败时 `out` 写 `{"code":"...","message":"..."}`，返回非 0；dyyl 把这个转成 `RuntimeError` + sentinel，脚本继续运行。
- 最小插件不读 credentials，`set_credentials` 直接返 0；如要读 credentials 详见 §7 与 §11.9。

### 2.3 构建为 .so

```bash
cd example-plugin
cargo build --release
# 产物：target/release/libexample.so（Linux）
```

### 2.4 用 server.js 本地分发测试

把构建产物按 dyyl 期望的目录布局拷到 `dist/plugins/<name>/<version>/<platform>/`，再写一个 `manifest.json`：

```bash
# 创建分发目录
mkdir -p dist/plugins/example/0.1.0/linux-x86_64

# 拷贝动态库
cp target/release/libexample.so dist/plugins/example/0.1.0/linux-x86_64/

# 计算 SHA256
sha256sum dist/plugins/example/0.1.0/linux-x86_64/libexample.so
```

写 `dist/plugins/example/manifest.json`：

```json
{
  "name": "example",
  "version": "0.1.0",
  "abi_version": 2,
  "dyyl_min": "0.2.0",
  "panic_mode": "abort",
  "commands": [
    {"name":"greet","arity":1,"brief":"Send a greeting"},
    {"name":"math.double","arity":1,"brief":"Double a number"}
  ],
  "platforms": [
    {
      "platform": "linux-x86_64",
      "url": "http://localhost:8951/plugins/example/0.1.0/linux-x86_64/libexample.so",
      "sha256": "<上面 sha256sum 输出的 hash>"
    }
  ]
}
```

启动本地分发服务器（[server.js](file:///workspace/server.js) 默认监听 8951 端口）：

```bash
node server.js
# 输出：dyyl install server running on http://0.0.0.0:8951
```

server.js 已经支持 `/plugins/<name>/manifest.json` 与 `/plugins/<name>/<version>/<platform>/<filename>` 两条路由（见 [server.js#L74-L128](file:///workspace/server.js#L74-L128)），把产物按上面的布局放好即可。

### 2.5 在 dyyl 脚本里调用

dyyl 解释器需要把官方源指向本地服务器（生产是 `https://l.dyyapp.com`，开发用 `http://localhost:8951`）。具体配置见 §9.5。写一个测试脚本 `test_example.dyyl`：

```dyyl
# 调用 example.greet
set $g, example.greet "World"
io.out $g

# 调用 example.math.double（多级命令路由，详见 §3.5）
set $d, example.math.double 21
io.out $d
```

运行（首次会触发下载安装）：

```bash
./target/release/dyyl test_example.dyyl
# 输出：
# Hello, World!
# 42
```

如要避免每次手写 manifest，可直接用发布脚本（详见 §9.4）：

```bash
./scripts/publish-plugin.sh example-plugin
# 自动构建、算 sha256、生成 manifest.json
```

完整的最小插件示例代码在 [tests/fixtures/example-plugin/](file:///workspace/tests/fixtures/example-plugin/)，dyyl 主仓的集成测试 [plugin_e2e_tests.rs](file:///workspace/tests/plugin_e2e_tests.rs) 用它做 dlopen 验证。

---

## 3. 架构与生命周期

### 3.1 插件调用数据流

dyyl 脚本里出现 `migpt.user.login "u", "p"` 时，整条调用链如下（参考 [plugin-ecosystem-design.md §3.3](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)）：

```text
脚本: migpt.user.login "u", "p"
  ↓
dispatch.rs: 命令名含点号且首段非已知命令族 → fallback 到 plugin::dispatch
  ↓
plugin.rs#dispatch_plugin_command: 按首个点号切分
  plugin_name = "migpt"
  sub = "user.login"  （可含多个点号）
  ↓
PluginManager.dispatch(name, sub, args, lang, line)  [src/runtime/plugin/mod.rs]
  ↓
（首次）install_plugin: fetch manifest → SHA256 校验 → 写入 XDG 目录
（再次）命中已加载 handle，直接跳到 step 4
  ↓
loader.dlopen + 解析 15 个 ABI 符号 + init(api_version) → handle
  ↓
（ABI v2）set_credentials(handle, credentials_json)
  ↓
on_load(handle)
  ↓
handle_command(handle, "user.login", args_json, &mut out_json)
  ↓
插件内 commands::dispatch → match "user.login" → user::login(state, args)
  ↓
返回 DyylValue → 编码为单个 Value 的 JSON → 写到 out_json
  ↓
dyyl 解析 out JSON → Value，返回 dispatch → 脚本拿到值
```

完整实现见 [src/runtime/plugin/mod.rs](file:///workspace/src/runtime/plugin/mod.rs)（PluginManager）与 [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs)（dlopen 与符号调用）。

### 3.2 加载/卸载时机

**加载**：dyyl 解释器在 `dispatch.rs` 的 fallback 分支首次遇到某插件名时调用 `PluginManager::dispatch`。该函数加锁 `loaded` map，若未加载则触发完整的 `load_plugin` 流程（见 [mod.rs#L122-L186](file:///workspace/src/runtime/plugin/mod.rs#L122-L186)）：

1. `registry::find_installed(name)` 查 XDG plugins 目录里是否已装。
2. 未装则 `install_plugin`：拉 manifest → 校验 ABI / dyyl_min / 平台条目 → 下载 → SHA256 校验 → 写 `lib<name>.so` 与 `plugin.toml`。
3. 读本地 `plugin.toml` 拿 commands/credentials 声明。
4. （仅 ABI v2）`assemble_credentials`：读 `credentials.toml` + 处理 `file`/`directory` 类型字段 → 拼成 JSON 字符串。
5. `PluginLoader::load(lib_path, name, credentials_json)`：dlopen → 验 API 版本 → `init(api_version)` → `set_credentials(handle, json)` → `on_load(handle)`。

**卸载**：脚本结束时 `Env` 被 drop，`PluginManager` 的 `loaded: Mutex<HashMap<.., LoadedPlugin>>` 一起 drop，每个 `LoadedPlugin` 的 `PluginLoader` 实现 `Drop`（见 [loader.rs#L210-L215](file:///workspace/src/runtime/plugin/loader.rs#L210-L215)），依次调：

1. `on_unload(handle)` —— 插件可在此清空内存缓存（OpenPGP 插件在 [lib.rs#L161-L170](file:///workspace/plugins/openpgp/src/lib.rs#L161-L170) 调 `state.clear_cache()`）。
2. `shutdown(handle)` —— 插件 `Box::from_raw(handle)` 回收内存。
3. `Library` drop → `dlclose`。

### 3.3 handle 的所有权

`init` 返回的 `*mut c_void` 是插件自己拥有的不透明指针。约定：

- 插件用 `Box::into_raw(Box::new(state))` 产生指针（OpenPGP 插件见 [lib.rs#L60-L64](file:///workspace/plugins/openpgp/src/lib.rs#L60-L64)），所有后续 ABI 函数把 `handle` 转回 `&mut PluginState`。
- `shutdown` 用 `Box::from_raw(handle)` 唯一回收（OpenPGP 插件见 [lib.rs#L175-L184](file:///workspace/plugins/openpgp/src/lib.rs#L175-L184)），保证释放恰好一次。
- 在 `init` 与 `shutdown` 之间，dyyl 持有这个指针但不解读其内容；插件的任意 ABI 函数都可以把它转回自己的状态类型。
- 最小插件可以用静态哨兵指针（如 [example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs) 的 `1 as *mut c_void`），但生产级插件应当用 `Box` 模式，否则无法持有跨调用状态。

### 3.4 panic = "abort" 为什么必须

Rust 默认 panic 行为是 unwind 栈，但跨 FFI 边界 unwind 是未定义行为（C 调用方不知道怎么处理 Rust 的 unwind）。具体后果：

- 可能直接 abort 进程（最常见的 Rust 实现策略）。
- 可能损坏 dyyl 的内部状态（mutex 中毒、内存破坏）。
- 不同 Rust 版本/compiler flags 下行为可能不同，不可预测。

设 `panic = "abort"` 后，panic 直接调 `abort()` 终止整个 dyyl 进程，行为确定但脚本无法继续。这把"未定义行为"降级为"进程终止"，是最重要的硬性约束。dyyl 在加载时只校验 manifest 的 `panic_mode` 字段（见 [manifest.rs#L13-L14](file:///workspace/src/runtime/plugin/manifest.rs#L13-L14)），不强制反编译产物验证，作者必须自行保证编译选项正确。

实际编码层面还应当：

- 在插件 `Cargo.toml` 加 `[lints.clippy]` 把 `unwrap_used`/`panic`/`indexing_slicing` 设为 deny（OpenPGP 插件就这么做，见 [Cargo.toml#L32-L38](file:///workspace/plugins/openpgp/Cargo.toml#L32-L38)）。
- 用 `unwrap_or`/`unwrap_or_else`/`?` 代替 `unwrap`/`expect`（OpenPGP 插件的 [codec.rs#L166-L169](file:///workspace/plugins/openpgp/src/codec.rs#L166-L169) 给了一个在 `CString::new` 不可失败场景下绕开 `unwrap` 的范例）。

---

## 4. C ABI 契约（参考手册）

### 4.1 15 符号全表

dyyl 加载插件时通过 `libloading::Library::get` 解析下列 15 个符号，缺一个就拒绝加载。函数指针类型定义见 [abi.rs#L62-L83](file:///workspace/src/runtime/plugin/abi.rs#L62-L83)，符号名列表见 [abi.rs#L87-L104](file:///workspace/src/runtime/plugin/abi.rs#L87-L104)。

| # | 符号名 | 签名 | 作用 | 内存约定 |
|---|---|---|---|---|
| 1 | `dyyl_plugin_get_api_version` | `() -> c_uint` | 返回插件编译时针对的 dyyl 插件 API 版本。dyyl 启动时校验（v1 或 v2 都接受，其它拒绝）。 | — |
| 2 | `dyyl_plugin_get_name` | `(*mut *mut c_char) -> c_int` | 通过出参返回插件名字符串，必须与 manifest.name 一致。 | 插件用 `CString::into_raw` 分配；dyyl 用完调 `free_string`。返回 0=成功。 |
| 3 | `dyyl_plugin_get_version` | `(*mut *mut c_char) -> c_int` | 写插件版本字符串。 | 同上。 |
| 4 | `dyyl_plugin_get_author` | `(*mut *mut c_char) -> c_int` | 写作者名（可空串）。 | 同上。 |
| 5 | `dyyl_plugin_get_description` | `(*mut *mut c_char) -> c_int` | 写描述（可空串）。 | 同上。 |
| 6 | `dyyl_plugin_init` | `(c_uint) -> *mut c_void` | 初始化。参数是 dyyl 当前 ABI 版本（v2）。返回插件 handle；NULL 表示失败。 | handle 由插件拥有，`shutdown` 回收。 |
| 7 | `dyyl_plugin_on_load` | `(*mut c_void) -> c_int` | 加载完成钩子（在 `set_credentials` 之后调用）。0=成功，非 0 dyyl 拒绝该插件并 abort。 | — |
| 8 | `dyyl_plugin_list_commands` | `(*mut c_void, *mut *mut c_char) -> c_int` | 输出 JSON 数组 `[{"name":"greet","arity":1,"brief":"..."}, ...]`。`name` 可含点号表多级子命令。dyyl 据此做存在性 + arity 校验。 | 插件分配，dyyl 调 `free_string`。 |
| 9 | `dyyl_plugin_get_command_help` | `(*mut c_void, *const c_char, *mut *mut c_char) -> c_int` | 返回单命令帮助字符串（`plugin.help <name> <cmd>` 用）。 | 同上。 |
| 10 | `dyyl_plugin_handle_command` | `(*mut c_void, *const c_char, *const c_char, *mut *mut c_char) -> c_int` | **核心调度**。参数依次为 handle、`cmd_name`（去掉插件名前缀，可含点号）、`args_json`（dyyl Value 数组的 JSON）、out 出参。0=成功，非 0=失败。 | 成功时 out 写单个 Value 的 JSON；失败时 out 写 `{"code":...,"message":...}`。dyyl 调 `free_string`。 |
| 11 | `dyyl_plugin_on_error` | `(*mut c_void, *const c_char, c_int, *const c_char) -> c_int` | dyyl 在 `handle_command` 返回非 0 后回调，插件可记录/清理。返回值忽略。 | — |
| 12 | `dyyl_plugin_on_unload` | `(*mut c_void) -> c_int` | 卸载前钩子。返回值忽略。插件应在此清空内存缓存。 | — |
| 13 | `dyyl_plugin_shutdown` | `(*mut c_void) -> ()` | 释放 handle。之后 dyyl 不再调用任何符号。插件应 `Box::from_raw` 唯一回收。 | — |
| 14 | `dyyl_plugin_free_string` | `(*mut c_char) -> ()` | 释放插件分配的字符串。dyyl 把通过出参拿到的指针回传给此函数。NULL 安全（应直接 return）。 | 插件用与分配时配对的回收方式（`CString::from_raw`）。 |
| 15 | `dyyl_plugin_set_credentials` | `(*mut c_void, *const c_char) -> c_int` | **ABI v2 新增**。在 `on_load` 前调用，传入凭证 JSON 字符串（结构详见 §7）。0=成功，非 0 dyyl 拒绝该插件。 | json 串由 dyyl 拥有，插件只读。 |

### 4.2 符号导出模板代码

每个符号都用 `#[no_mangle] extern "C"`，确保 Rust mangling 不影响 dlsym 查找。最小模板（取自 [example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs)）：

```rust
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};

#[no_mangle]
pub extern "C" fn dyyl_plugin_get_api_version() -> c_uint {
    2  // ABI v2
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_init(_api_version: c_uint) -> *mut c_void {
    // 生产级插件用 Box::into_raw 模式（详见 §11.3）
    let state = Box::new(MyState::default());
    Box::into_raw(state) as *mut c_void
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_handle_command(
    handle: *mut c_void,
    cmd: *const c_char,
    args: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    // 安全地把 handle 转回自己的状态类型
    let state: &mut MyState = unsafe { &mut *(handle as *mut MyState) };
    let cmd_str = unsafe { CStr::from_ptr(cmd) }.to_str().unwrap_or("");
    let args_str = unsafe { CStr::from_ptr(args) }.to_str().unwrap_or("[]");
    // ... 调用具体子命令，把结果 JSON 写到 *out
    0
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_shutdown(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    unsafe { let _ = Box::from_raw(handle as *mut MyState); }
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { let _ = CString::from_raw(ptr); }
}
```

完整 15 符号的真实实现见 [plugins/openpgp/src/lib.rs](file:///workspace/plugins/openpgp/src/lib.rs)。

### 4.3 字符串内存约定

- **出参字符串**（`out: *mut *mut c_char`）：插件用 `CString::new(s).into_raw()` 分配，写到 `*out`；dyyl 用完调 `dyyl_plugin_free_string(ptr)`，插件用 `CString::from_raw(ptr)` 回收。两端必须用同一种分配方式（都用 Rust 标准库的 `CString`），不能一端用 `malloc` 一端用 `CString`。
- **入参字符串**（`*const c_char`，如 `cmd`、`args_json`、`creds_json`）：由 dyyl 拥有，插件只读，调用返回后 dyyl 保证指针有效。插件不要 free 这些指针。
- **NUL 处理**：`CString::new` 遇到内嵌 NUL 字节会失败。OpenPGP 插件用 `cstring_from_str` 帮助函数过滤 NUL（见 [codec.rs#L166-L169](file:///workspace/plugins/openpgp/src/codec.rs#L166-L169)），保证 `CString::new` 不失败、不触发 `unwrap`。
- **NULL 处理**：所有 ABI 函数都应当对 NULL 入参做防御（OpenPGP 插件在 [lib.rs#L106-L122](file:///workspace/plugins/openpgp/src/lib.rs#L106-L122) 检查 `handle.is_null()`/`cmd.is_null()`/`args.is_null()`，分别走兜底分支）。

### 4.4 ABI 版本兼容（v1 vs v2）

dyyl 当前同时支持 ABI v1 与 v2（见 [loader.rs#L46-L52](file:///workspace/src/runtime/plugin/loader.rs#L46-L52)）：

| ABI 版本 | 符号数 | `set_credentials` | 行为 |
|---|---|---|---|
| v1 | 14（无 `set_credentials`） | 不导出 | dyyl 跳过 `set_credentials` 调用，插件无法接收 credentials JSON（始终 `None`）。 |
| v2 | 15（含 `set_credentials`） | 必须导出 | dyyl 在 `on_load` 前调 `set_credentials(handle, json)`；插件可解析 JSON 拿到凭证字段。 |

dyyl 主仓的 `assemble_credentials` 仅在 `manifest.abi_version >= 2` 时构造 credentials JSON（见 [mod.rs#L166-L170](file:///workspace/src/runtime/plugin/mod.rs#L166-L170)），v1 插件直接传 `None` 跳过符号查找。新插件应使用 v2，以便使用 credentials 系统（§7）。

---

## 5. Value JSON 编解码

### 5.1 6 种 Value 类型

dyyl 内部的 `Value` 枚举有 6 个变体（见 [src/runtime/value.rs](file:///workspace/src/runtime/value.rs)）。跨 FFI 时统一编码为 JSON 对象，`type` 字段区分：

| Value 类型 | JSON 形式 | 说明 |
|---|---|---|
| `Num` | `{"type":"num","value":"123"}` | 数值用字符串形式，保留任意精度（CasNumber 支持大整数、分数、根式）。 |
| `Str` | `{"type":"str","value":"hello"}` | 普通字符串。 |
| `Expr` | `{"type":"expr","value":"1⅔"}` | 符号表达式。插件侧按 `Num` 处理（best-effort 解析）。 |
| `Empty` | `{"type":"empty"}` | 占位空值（`_` / `empty` 关键字）。 |
| `List` | `{"type":"list","value":[...]}` | 数组，元素是 Value 对象。 |
| `Dict` | `{"type":"dict","value":[{"key":{...},"val":{...}},...]}` | 保序 KV 对，`key`/`val` 各是 Value 对象。 |

完整 codec 实现在 dyyl 侧 [src/runtime/plugin/value_codec.rs](file:///workspace/src/runtime/plugin/value_codec.rs)，对称的插件侧实现见 [plugins/openpgp/src/codec.rs](file:///workspace/plugins/openpgp/src/codec.rs)。

### 5.2 dyyl → 插件 args_json 数组格式

`handle_command` 的 `args` 参数是 dyyl 把脚本的实参列表编成的 JSON 数组。每个元素是一个 Value 对象。例：脚本 `migpt.greet "Alice"` 的 `args_json` 是：

```json
[{"type":"str","value":"Alice"}]
```

多参数时按顺序排列，dyyl 在 [mod.rs#L103](file:///workspace/src/runtime/plugin/mod.rs#L103) 调 `values_to_json_array(args)` 完成。

### 5.3 插件 → dyyl out_json 单值格式

`handle_command` 的 `out` 出参写**单个 Value** 的 JSON 对象（不是数组）。例：返回字符串 `"Hello, Alice!"` 时写：

```json
{"type":"str","value":"Hello, Alice!"}
```

dyyl 在 [mod.rs#L112](file:///workspace/src/runtime/plugin/mod.rs#L112) 调 `value_from_json(&result_json)` 解析回 `Value`。失败时 `out` 改写错误对象（见 §4.1 第 10 行）。

### 5.4 num 用字符串的原因

dyyl 的 `CasNumber` 支持任意大整数、分数、根式、常量符号（如 `π`）。JSON number 是 IEEE 754 double，装不下这些精度。把 num 编码为字符串保持精确，与 dyyl 主仓的 `mcm` 协议 `McmArg::Str` 策略一致。插件侧的 `DyylValue::Num(String)` 直接持有字符串，需要时再 `parse::<i64>()` 或交给具体数值库处理（OpenPGP 插件大部分命令不涉及 num，少数命令如 `key.delete` 用 `"1"`/`"0"` 表成功/失败）。

### 5.5 Rust 端 DyylValue 枚举

插件侧不必依赖 dyyl runtime crate，自己镜像一份枚举即可。OpenPGP 插件的定义（[codec.rs#L19-L26](file:///workspace/plugins/openpgp/src/codec.rs#L19-L26)）：

```rust
/// A dyyl value, mirrored from the host for cross-FFI marshalling.
#[derive(Debug, Clone)]
pub enum DyylValue {
    Num(String),
    Str(String),
    Empty,
    List(Vec<DyylValue>),
    Dict(Vec<(DyylValue, DyylValue)>),  // 与 dyyl 一致：保序 KV 对
}
```

`Dict` 用 `Vec<(K, V)>` 而非 `HashMap` 是为了与 dyyl 主仓的 `Value::Dict(Vec<(Value, Value)>)` 对齐，保序且允许重复键（dyyl 当前语义如此）。

### 5.6 编解码函数

插件侧的编解码 API（参考 [codec.rs#L58-L160](file:///workspace/plugins/openpgp/src/codec.rs#L58-L160)）：

```rust
/// 解析 dyyl 传来的 args JSON 数组
pub fn decode_args(json: &str) -> Result<Vec<DyylValue>, String> { ... }

/// 把单个 DyylValue 编码为 JSON 字符串
pub fn encode_value(v: &DyylValue) -> String { ... }

/// 编码并写到 out 出参（FFI 用）
pub fn encode_out(out: *mut *mut c_char, v: &DyylValue) { ... }
```

`Expr` 类型在插件侧按 `Num` 处理（best-effort 解析），与 dyyl 主仓的 [value_codec.rs#L62-L66](file:///workspace/src/runtime/plugin/value_codec.rs#L62-L66) 行为对称：dyyl 收到 `expr` JSON 时也尝试 `parse` 为 num。

### 5.7 嵌套结构示例

脚本里构造一个嵌套 dict：

```dyyl
set $d, dict.create "users", (list.create "alice", "bob")
```

传给插件的 `args_json` 形如：

```json
[
  {"type":"str","value":"users"},
  {"type":"list","value":[
    {"type":"str","value":"alice"},
    {"type":"str","value":"bob"}
  ]}
]
```

插件返回一个 dict of dict 时，`out_json` 形如：

```json
{
  "type": "dict",
  "value": [
    {
      "key": {"type":"str","value":"signer"},
      "val": {"type":"dict","value":[
        {"key":{"type":"str","value":"uid"},"val":{"type":"str","value":"alice <a@x>"}},
        {"key":{"type":"str","value":"fp"}, "val":{"type":"str","value":"ABCD1234"}}
      ]}
    }
  ]
}
```

OpenPGP 插件的 `verify` 命令返回 `{valid, signer_uid, signer_fp}` dict 就是这种结构（详见 §11.7 与 [verify.rs#L110-L125](file:///workspace/plugins/openpgp/src/commands/verify.rs#L110-L125)）。

---

## 6. Manifest 与 plugin.toml

### 6.1 远程 manifest.json schema 全字段

远程清单 URL 约定：`https://l.dyyapp.com/plugins/<name>/manifest.json`（开发用 `http://localhost:8951/plugins/<name>/manifest.json`）。完整字段定义在 [manifest.rs#L7-L23](file:///workspace/src/runtime/plugin/manifest.rs#L7-L23)（`RemoteManifest` 结构）：

```jsonc
{
  "name": "openpgp",                       // 必填，与 lib.name 一致
  "version": "0.1.0",                      // 必填，semver 字符串
  "abi_version": 2,                        // 必填，1 或 2
  "dyyl_min": "0.2.0",                     // 必填，要求的最低 dyyl 版本
  "panic_mode": "abort",                   // 可选，默认 "abort"
  "commands": [                            // 可选，默认空数组
    {
      "name": "key.generate",              // 必填，可含点号表多级命令
      "arity": 2,                          // 必填，参数个数
      "brief": "Generate a new keypair"    // 可选，单行描述
    }
  ],
  "platforms": [                           // 必填，至少一个平台条目
    {
      "platform": "linux-x86_64",          // 形如 "<os>-<arch>"
      "url": "http://localhost:8951/plugins/openpgp/0.1.0/linux-x86_64/libopenpgp.so",
      "sha256": "<64 位 hex>"              // 必填，dyyl 下载后校验
    }
  ],
  "has_locales": false,                    // 可选，默认 false。true 表示插件自带 locales/ 目录
  "credentials": {                         // 可选，凭证字段声明（详见 §7）
    "fields": [
      {"name":"passphrase","type":"string","secret":true,"description":"..."}
    ]
  }
}
```

### 6.2 本地 plugin.toml 副本

dyyl 安装时把远程 manifest 的内容（加上安装元数据）写入本地 `plugin.toml`，路径是 `<xdg_data>/dyyl/plugins/<name>/<version>/plugin.toml`（见 [store.rs#L37-L39](file:///workspace/src/runtime/plugin/store.rs#L37-L39)）。结构定义在 [manifest.rs#L48-L60](file:///workspace/src/runtime/plugin/manifest.rs#L48-L60)（`LocalPluginToml`）：

```toml
name = "openpgp"
version = "0.1.0"
abi_version = 2
dyyl_min = "0.2.0"
panic_mode = "abort"

[[commands]]
name = "key.generate"
arity = 2
brief = "Generate a new Ed25519/Curve25519 keypair, store in keyring, return fingerprint"

[[commands]]
name = "key.import"
arity = 1
brief = "Import armored public or private key into keyring"

# ... 其余命令 ...

[installed]
source_url = "http://localhost:8951/plugins/openpgp/0.1.0/linux-x86_64/libopenpgp.so"
sha256 = "abc..."
installed_at = "2026-07-14T10:30:00Z"
dyyl_version = "0.2.0"
```

`installed` 段是 dyyl 安装时填的元数据（见 [mod.rs#L356-L372](file:///workspace/src/runtime/plugin/mod.rs#L356-L372) 的 `build_local_toml` 函数）。`commands` 段从远程 manifest 透传。

### 6.3 commands[].name 含点号的多级命令约定

`commands[].name` 可以含点号，表示多级子命令。dyyl 在校验时按完整名匹配（见 [manifest.rs#L97-L99](file:///workspace/src/runtime/plugin/manifest.rs#L97-L99) 的 `find_command`）。

调用语法对照（dyyl 切分规则见 [plugin.rs#L26-L34](file:///workspace/src/runtime/cmd/plugin.rs#L26-L34)）：

| 脚本里的命令 | plugin_name | 传给 handle_command 的 cmd_name |
|---|---|---|
| `openpgp.encrypt` | openpgp | `encrypt` |
| `openpgp.key.generate` | openpgp | `key.generate` |
| `openpgp.gpg.key.list` | openpgp | `gpg.key.list` |

manifest 里 `commands[].name` 与 `list_commands` 输出的 name 都用此形式（去掉插件名前缀）。OpenPGP 插件的完整 30 条命令清单在 [command_list.json](file:///workspace/plugins/openpgp/command_list.json)。

### 6.4 platforms 多平台条目

`platforms` 是数组，每个条目描述一个平台产物。dyyl 在下载前调 [store.rs#L60-L78](file:///workspace/src/runtime/plugin/store.rs#L60-L78) 的 `current_platform()` 拿当前平台字符串（如 `linux-x86_64`），在 `platforms` 里找匹配条目；找不到则报 `plugin_platform_unavailable` 错误（见 [mod.rs#L264-L286](file:///workspace/src/runtime/plugin/mod.rs#L264-L286)）。

平台字符串格式是 `<os>-<arch>`：

| os | arch | 字符串 |
|---|---|---|
| linux | x86_64 | `linux-x86_64` |
| linux | aarch64 | `linux-aarch64` |
| macos | aarch64 | `macos-aarch64` |
| windows | x86_64 | `windows-x86_64` |

跨平台发布时，每个平台的产物都要单独计算 SHA256（详见 §9.3 与 §9.6）。

### 6.5 abi_version / dyyl_min / panic_mode

| 字段 | 校验逻辑 | 失败行为 |
|---|---|---|
| `abi_version` | dyyl 当前支持 1 与 2；其它值拒绝（见 [mod.rs#L250-L262](file:///workspace/src/runtime/plugin/mod.rs#L250-L262)）。 | `plugin_abi_mismatch` RuntimeError + sentinel。 |
| `dyyl_min` | dyyl 当前实现未强制检查版本号（在 v0.2.0 时尚无版本比较逻辑）；作者应声明 `"0.2.0"`。i18n 文案 `plugin.dyyl_min_unmet` 已就绪（见 [zh.json](file:///workspace/locales/zh.json#L84)）。 | 预留：未来会拒绝加载。 |
| `panic_mode` | 字段值目前仅记录，未强制校验编译选项。约定值 `"abort"`，作者必须自行保证 `[profile.release] panic = "abort"`。 | 无运行时强制；UB 风险由文档警告（§12.2）。 |

### 6.6 has_locales 字段

`has_locales: true` 表示插件自带 `locales/` 目录（含 `en.json`、`zh.json`）。dyyl 加载时调 `i18n::register_plugin(name, en, zh)` 把插件的消息表注册到全局 `MessageStore`（见 [i18n.rs#L97-L103](file:///workspace/src/i18n.rs#L97-L103)）。键名约定 `<plugin_name>.<key>`，dyyl 在 `t()` 时按 key 的首段路由到插件表（见 [i18n.rs#L131-L159](file:///workspace/src/i18n.rs#L131-L159) 的 `lookup_template`）。详见 §8。

---

## 7. Credentials 系统

### 7.1 credentials.toml 结构

文件路径：`~/.config/dyyl/credentials.toml`（与 `config.toml` 同目录，XDG config；见 [credentials.rs#L150-L155](file:///workspace/src/credentials.rs#L150-L155)）。结构定义在 [credentials.rs#L47-L53](file:///workspace/src/credentials.rs#L47-L53)：

```toml
# dyyl 内置 AI 凭证
[ai]
provider = "anthropic"           # openai-chat / openai-response / anthropic
api_key = "sk-..."
model = "claude-..."
base_url = ""                    # 可选，空表示官方端点
auto_system_prompt = ""          # 可选

# 各插件的 string 字段
[plugin.openpgp]
passphrase = "default-pass"
default_key = "ABCD1234EF567890"

[plugin.migpt]
api_token = "xxx"
```

`[ai]` 段是 dyyl 内置 AI 凭证（[credentials.rs#L34-L44](file:///workspace/src/credentials.rs#L34-L44)）。`[plugin.<name>]` 段是各插件的 `string` 类型凭证字段，dyyl 加载插件时把整段读出来传给 `set_credentials`（详见 §7.5）。

### 7.2 manifest credentials.fields 声明

插件在远程 manifest 的 `credentials.fields` 数组里声明自己需要的字段。结构见 [manifest.rs#L72-L88](file:///workspace/src/runtime/plugin/manifest.rs#L72-L88)：

```jsonc
"credentials": {
  "fields": [
    {
      "name": "passphrase",          // 字段名，注入到 JSON 的 key
      "type": "string",              // string / file / directory（详见 §7.3）
      "secret": true,                // 标记敏感字段（用于未来 no-echo 输入）
      "description": "Default passphrase for encrypted private keys"
    }
  ]
}
```

OpenPGP 插件声明了 3 个字段（[plugin.toml.in#L157-L173](file:///workspace/plugins/openpgp/plugin.toml.in#L157-L173)）：`passphrase`（string, secret）、`default_key`（string）、`__credentials_dir`（directory）。

### 7.3 三种字段类型

dyyl 在 `creds_inject.rs` 的 `resolve_field` 函数（[creds_inject.rs#L47-L78](file:///workspace/src/runtime/plugin/creds_inject.rs#L47-L78)）按 `type` 字段分发：

| type | dyyl 行为 | JSON 注入值 |
|---|---|---|
| `"string"`（默认） | 读 `credentials.toml` 的 `[plugin.<name>]` 段对应字段。 | 字符串原值。字段缺失触发交互提示（详见 §7.6）。 |
| `"file"` | 读 `<credentials_dir>/<field>` 文件内容。文件不存在则注入空字符串 + debug 警告。 | 文件内容字符串（UTF-8）。 |
| `"directory"` | dyyl 不创建（只在首次注入时由 `ensure_credentials_dir` 创建 `credentials_dir` 本身，权限 0700）。注入绝对路径。 | 目录绝对路径字符串。 |

### 7.4 __credentials_dir 自动注入机制

无论插件 manifest 是否显式声明 `__credentials_dir` 字段，dyyl 都会自动注入此字段（见 [creds_inject.rs#L31-L34](file:///workspace/src/runtime/plugin/creds_inject.rs#L31-L34)）。值是 `<xdg_data>/dyyl/credentials.d/<plugin_name>/`（见 [credentials.rs#L280-L286](file:///workspace/src/credentials.rs#L280-L286) 的 `credentials_dir_for_plugin`）。

dyyl 在构造 credentials JSON 前会调用 `ensure_credentials_dir`（[creds_inject.rs#L80-L89](file:///workspace/src/runtime/plugin/creds_inject.rs#L80-L89)）确保该目录存在，权限 0700（仅 Unix）。

OpenPGP 插件正是依赖此机制定位钥匙串目录（[creds.rs#L42-L44](file:///workspace/plugins/openpgp/src/creds.rs#L42-L44) 把 `__credentials_dir` 写入 `state.credentials_dir`）。

### 7.5 set_credentials ABI 调用时机

dyyl 在 `PluginLoader::load` 内部的调用顺序（[loader.rs#L31-L92](file:///workspace/src/runtime/plugin/loader.rs#L31-L92)）：

1. `dlopen` 打开动态库。
2. `get_api_version` 校验 ABI 版本兼容。
3. `init(api_version)` 拿 handle。
4. （仅 ABI v2）`set_credentials(handle, credentials_json)`。
5. `on_load(handle)`。

`set_credentials` 在 `on_load` **之前**调用，这样 `on_load` 里就能访问已注入的凭证状态。OpenPGP 插件在 `apply_credentials`（[creds.rs#L23-L46](file:///workspace/plugins/openpgp/src/creds.rs#L23-L46)）把 JSON 解析到 `PluginState`，`on_load` 直接返回 0（无额外初始化）。

credentials JSON 形如（OpenPGP 插件实际收到的）：

```json
{
  "passphrase": "default-pass",
  "default_key": "ABCD1234EF567890",
  "__credentials_dir": "/home/user/.local/share/dyyl/credentials.d/openpgp"
}
```

### 7.6 交互式提示流程

dyyl 在 `assemble_credentials`（[mod.rs#L188-L231](file:///workspace/src/runtime/plugin/mod.rs#L188-L231)）里检查：如果 manifest 声明了 `type:"string"` 字段，但 `credentials.toml` 的 `[plugin.<name>]` 段缺该字段，调 `ensure_plugin_credentials`（[credentials.rs#L294-L325](file:///workspace/src/credentials.rs#L294-L325)）触发交互式提示：

1. stderr 输出 `plugin.credential_prompt_header` 消息（如 `[dyyl] 插件 'openpgp' 需要凭证，请输入:`，见 [zh.json#L106](file:///workspace/locales/zh.json#L106)）。
2. 对每个缺失字段：stderr 输出描述（`field.description` 或字段名），从 stdin 读一行。
3. 写回 `credentials.toml` 的 `[plugin.<name>]` 段。
4. 重新读 `credentials.toml`，继续构造 credentials JSON。

`file` 与 `directory` 类型字段不触发提示（由 dyyl 自动注入）。

stdin EOF（如非交互环境）会返回 `Err`，整个 `load_plugin` 失败，脚本得到 RuntimeError + sentinel。

### 7.7 权限约定

| 路径 | 权限 | 行为 |
|---|---|---|
| `<xdg_data>/dyyl/credentials.d/<plugin>/` | 0700（仅属主读写执行） | dyyl 创建目录时设置（见 [creds_inject.rs#L83-L87](file:///workspace/src/runtime/plugin/creds_inject.rs#L83-L87)）。已存在目录权限非 0700 不修正。 |
| `<credentials_dir>/<plugin>/keys/*.sec.asc` | 0600（仅属主读写） | 插件自己写私钥文件时设置（OpenPGP 插件在 [keyring.rs#L91-L98](file:///workspace/plugins/openpgp/src/keyring.rs#L91-L98) 的 `write_key_file`）。 |
| `~/.config/dyyl/credentials.toml` | 默认 0644 | dyyl 不强制修正权限；`--debug` 时权限过松仅警告。 |

### 7.8 大型/动态凭据模式（用 OpenPGP 钥匙串作范例）

`string` 字段适合小型、固定凭证（如 API token、passphrase）。当凭证数据较大（如 armored 私钥动辄几 KB 含换行）或需要动态增删（如钥匙串）时，应当用 `file` / `directory` 类型：

- `file`：插件要求 dyyl 注入某文件的内容字符串。如 `revocation_cert` 字段指向 `credentials.d/<plugin>/revocation_cert`，dyyl 读文件内容注入。文件不存在则注入空串 + debug 警告。
- `directory`：插件要求 dyyl 注入某目录的绝对路径，插件自己管理目录内容。`__credentials_dir` 就是这种类型，每插件自动注入。

OpenPGP 插件的钥匙串布局（设计文档 §5.4）：

```text
credentials.d/openpgp/
  keys/
    ABCD1234EF567890.pub.asc      # 公钥（按指纹命名）
    ABCD1234EF567890.sec.asc      # 私钥（passphrase 加密）
  index.json                       # 插件维护的索引：fp → uid/created/has_secret
```

插件在 `credentials_dir` 下自管 `keys/<fp>.{pub,sec}.asc` 与 `index.json`（实现见 [keyring.rs](file:///workspace/plugins/openpgp/src/keyring.rs)）。详见 §11.9。

---

## 8. i18n（插件双语）

### 8.1 locales/en.json + locales/zh.json 结构

插件自带的 `locales/en.json` 与 `locales/zh.json` 是扁平 key-value JSON，键名约定 `<plugin_name>.<key>`。例：

```json
// locales/en.json
{
  "openpgp.key_generated": "Key generated: {fp}",
  "openpgp.verify_ok": "Signature valid (signed by {signer})"
}

// locales/zh.json
{
  "openpgp.key_generated": "已生成密钥：{fp}",
  "openpgp.verify_ok": "签名有效（由 {signer} 签署）"
}
```

占位符语法 `{name}` 与 dyyl 主仓一致（见 [i18n.rs#L164-L171](file:///workspace/src/i18n.rs#L164-L171) 的 `interpolate` 函数）。

### 8.2 manifest has_locales 字段

manifest 的 `has_locales: true` 表示插件自带 locales 目录。dyyl 加载时调 `i18n::register_plugin(name, en, zh)`（[i18n.rs#L99-L103](file:///workspace/src/i18n.rs#L99-L103)）把这两个表注册到全局 `MessageStore`。

### 8.3 register_plugin 注册流程

`register_plugin(name, en, zh)` 把插件表存进 `MessageStore.plugins: Mutex<HashMap<String, PluginMessages>>`（[i18n.rs#L57-L61](file:///workspace/src/i18n.rs#L57-L61)）。之后 `t(lang, "openpgp.key_generated", args)` 时，`lookup_template`（[i18n.rs#L131-L159](file:///workspace/src/i18n.rs#L131-L159)）按 key 首段路由：

1. 取 key 首段（如 `openpgp.key_generated` 取 `openpgp`）作为 plugin_name。
2. 在 `plugins` map 里查 plugin_name。
3. 命中：按 lang 取 `pm.en`/`pm.zh`，找不到时 zh fallback 到 en。
4. 未命中或 key 不以插件名开头：走 dyyl 主表 `en`/`zh`，zh 缺失时 fallback 到 en + stderr 警告。

### 8.4 键命名约定

约定：`<plugin_name>.<key>`。如 `openpgp.key_generated`、`migpt.login_success`。这与 dyyl 主仓的命名一致（`plugin.install_success`、`ai.ask_failed` 等，见 [en.json](file:///workspace/locales/en.json)）。命名空间避免冲突，按插件名隔离。

### 8.5 zh 缺失 fallback 到 en

dyyl 主仓的 zh fallback 行为（[i18n.rs#L145-L150](file:///workspace/src/i18n.rs#L145-L150)）：

```rust
Lang::Zh => self.zh.get(key).or_else(|| {
    eprintln!("i18n warning: zh translation missing for '{key}', falling back to en");
    self.en.get(key)
}),
```

插件的 fallback 在 [i18n.rs#L136-L139](file:///workspace/src/i18n.rs#L136-L139)：

```rust
Lang::Zh => pm.zh.get(key).or_else(|| pm.en.get(key)),
```

注意：插件 zh 缺失时**静默** fallback 到 en（不打印警告），与 dyyl 主表的 fallback 行为略有差异。如果插件作者希望发现 zh 缺失，应当自己写测试覆盖（参考 dyyl 主仓的 `missing_translations` 函数 [i18n.rs#L117-L128](file:///workspace/src/i18n.rs#L117-L128)）。

### 8.6 插件返回的 message/brief/help 不被 dyyl 翻译

约定：插件 `handle_command` 失败时 out 写的 `message` 字段是**插件自己的原文**，dyyl **不翻译**。dyyl 翻译的是它自己生成的消息（manifest 校验失败、SHA256 不符、网络错误、ABI 不兼容、未知子命令前的路由提示等）。

插件作者负责自己消息的双语：要么根据 dyyl 传来的 `lang` 选择（但 dyyl 当前不传 lang 给插件，这是设计约束），要么直接用英文（最常见做法）。`code` 字段是机器可读枚举，不翻译。`brief` 字段（manifest 里）和 `get_command_help` 返回的帮助字符串也是插件自己的原文，dyyl 不翻译。

---

## 9. 构建、发布与分发

### 9.1 Cargo.toml 配置

OpenPGP 插件的 [Cargo.toml](file:///workspace/plugins/openpgp/Cargo.toml) 是生产级范例：

```toml
[package]
name = "openpgp-plugin"
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-or-later"

[lib]
name = "openpgp"                          # 产物 libopenpgp.so / openpgp.dll
crate-type = ["cdylib", "rlib"]           # cdylib 必填；rlib 让 tests/ 能内联测试

[dependencies]
sequoia-openpgp = { version = "2", default-features = true, features = ["compression-deflate"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
shell-words = "1"                          # gpg.exec 参数 split
which = "6"                                # gpg.detect 找 gpg 路径
base64 = "0.22"                            # armor/dearmor
tempfile = "3"                             # gpg.verify 分离验签临时文件

[profile.release]
panic = "abort"                            # 跨 FFI panic 必须 abort（§3.4 / §12.2）
opt-level = 3
lto = true

[lints.rust]
unsafe_op_in_unsafe_fn = "deny"

[lints.clippy]
all = { level = "deny", priority = -1 }
unwrap_used = "deny"                       # 防 panic
panic = "deny"
todo = "deny"
unimplemented = "deny"
indexing_slicing = "deny"                  # 强制用 .get() 显式处理 None
```

要点：

- `crate-type = ["cdylib"]` 必填，决定产物是动态库。
- 加 `"rlib"` 让 `tests/` 目录能内联测试 codec/keyring 等内部模块（OpenPGP 插件的 [tests/key_tests.rs](file:///workspace/plugins/openpgp/tests/key_tests.rs) 就 `use openpgp::codec::DyylValue;`）。
- `[profile.release] panic = "abort"` 是硬性约束。
- `[lints.clippy]` 与 dyyl 主仓一致，避免 `unwrap`/`panic`/`indexing_slicing` 触发 panic。

### 9.2 单平台构建

```bash
cd plugins/openpgp
cargo build --release
# 产物：target/release/libopenpgp.so（Linux）
# 或 target/release/libopenpgp.dylib（macOS）
# 或 target/release/openpgp.dll（Windows）
```

### 9.3 跨平台构建（cargo build --target）

用 `--target` 指定 rustc target triple。需要先 `rustup target add <triple>` 装好目标平台标准库。

```bash
# Linux x86_64 → Linux aarch64
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
# 产物：target/aarch64-unknown-linux-gnu/release/libopenpgp.so

# macOS aarch64（Apple Silicon）
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
# 产物：target/aarch64-apple-darwin/release/libopenpgp.dylib

# Windows x86_64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
# 产物：target/x86_64-pc-windows-gnu/release/openpgp.dll
```

注意：跨平台构建若有 C 依赖（如 sequoia 默认 backend 用 nettle，需要 C 库），交叉编译复杂度上升。OpenPGP 插件用 `default-features = true` 接受 sequoia 默认 backend（可能引入 nettle 系统依赖）；如要纯 Rust，可改 `default-features = false` + `features = ["compression-deflate"]`（实现时验证 backend 是否仍可用）。

发布脚本 `--target` 选项把多次构建合并到 manifest.platforms 数组（详见 §9.4）。

### 9.4 scripts/publish-plugin.sh 用法

[publish-plugin.sh](file:///workspace/scripts/publish-plugin.sh) 接受两种调用形式：

```bash
# 形式 1：从源码目录构建并发布（推荐，自动生成完整 manifest）
./scripts/publish-plugin.sh plugins/openpgp
./scripts/publish-plugin.sh plugins/openpgp --target aarch64-unknown-linux-gnu

# 形式 2：直接指定已构建产物（最小 manifest，abi_version=1，无 commands/credentials）
./scripts/publish-plugin.sh openpgp 0.1.0 /path/to/libopenpgp.so
```

形式 1 的行为（[publish-plugin.sh#L14-L168](file:///workspace/scripts/publish-plugin.sh#L14-L168)）：

1. 读 `plugin.toml.in`（用 Python `tomllib` 解析）拿 name/version/abi_version/dyyl_min/panic_mode/commands/credentials 字段。
2. `cargo build --release [--target <t>]` 构建产物。
3. 找 `target/release/lib<name>.so`（或 `.dylib`/`.dll`）。
4. 检测当前平台（`uname -s` + `uname -m`）拼 `<os>-<arch>`。
5. 拷产物到 `dist/plugins/<name>/<version>/<platform>/<filename>`。
6. 计算 SHA256。
7. 生成 `dist/plugins/<name>/manifest.json`，url 字段根据 `DYRL_DIST_HOST` 环境变量切换：

```bash
# 开发环境（默认）
DYRL_DIST_HOST=http://localhost:8951 ./scripts/publish-plugin.sh plugins/openpgp

# 生产环境
DYRL_DIST_HOST=https://l.dyyapp.com ./scripts/publish-plugin.sh plugins/openpgp
```

跨平台发布需要多次调用脚本（每次 `--target <t>`），手动合并 manifest.platforms 数组。当前脚本是覆盖式写 manifest（每次重新生成），跨平台合并需作者自己拼。

### 9.5 server.js 本地分发

[server.js](file:///workspace/server.js) 是 Node.js 实现的本地分发服务器，监听 8951 端口。两条插件路由：

- `GET /plugins/<name>/manifest.json`：返回 `dist/plugins/<name>/manifest.json`（[server.js#L80-L98](file:///workspace/server.js#L80-L98)）。
- `GET /plugins/<name>/<version>/<platform>/<filename>`：返回对应产物文件（[server.js#L101-L128](file:///workspace/server.js#L101-L128)）。

两条路由都做了路径穿越防护（拒绝含 `..` 的路径）。其它路径返回 403。

dyyl 解释器要从本地服务器拉插件，需要把官方源 `https://l.dyyapp.com` 改为 `http://localhost:8951`。具体配置（环境变量或 config 字段）实现细节见 dyyl 主仓的 fetch.rs（[src/runtime/plugin/fetch.rs](file:///workspace/src/runtime/plugin/fetch.rs)）。

### 9.6 SHA256 校验流程

dyyl 下载产物后用 SHA256 校验完整性，流程在 [mod.rs#L289-L300](file:///workspace/src/runtime/plugin/mod.rs#L289-L300)：

1. 从 manifest 的 `platforms[].url` 下载字节流。
2. 计算 SHA256。
3. 与 `platforms[].sha256` 比对（[fetch.rs](file:///workspace/src/runtime/plugin/fetch.rs) 的 `download_and_verify`）。
4. 不符 → `plugin_sha256_mismatch` RuntimeError + sentinel（i18n 文案见 [zh.json#L86](file:///workspace/locales/zh.json#L86)）。
5. 符合 → 写入 `lib<name>.so` 与 `plugin.toml` 到 XDG 目录。

发布端计算 SHA256 的命令（脚本里 [publish-plugin.sh#L113](file:///workspace/scripts/publish-plugin.sh#L113)）：

```bash
sha256sum dist/plugins/<name>/<version>/<platform>/<filename> | cut -d' ' -f1
```

把 64 位 hex 字符串填到 manifest 的 `platforms[].sha256` 字段。

### 9.7 版本号与 ABI 版本策略

- **插件版本号**：用 semver `MAJOR.MINOR.PATCH`。dyyl 当前不强制 semver 校验，只按字典序选最大版本（[registry.rs#L75-L82](file:///workspace/src/runtime/plugin/registry.rs#L75-L82) 的 `find_installed` 用 `max_by_key(version)`）。
- **ABI 版本**：与 dyyl 主仓的 `DYRL_API_VERSION`（当前 2，见 [abi.rs#L24](file:///workspace/src/runtime/plugin/abi.rs#L24)）匹配。dyyl 同时接受 v1 与 v2 插件（[loader.rs#L46-L52](file:///workspace/src/runtime/plugin/loader.rs#L46-L52)）；其它版本拒绝。
- **升级策略**：dyyl 当前不锁版本，每次 `dyyl update <name>` 拉最新 manifest，下载安装到新版本目录。旧版本目录残留（可后续清理）。
- **sequoia 主版本升级**：插件内部依赖升级不影响 ABI（plugin 与 dyyl 之间只有 C ABI 接触面），插件 minor 版本 +1 即可。

---

## 10. 测试插件

### 10.1 插件 crate 独立单元测试

插件 crate 自己的 `tests/` 目录用 `cargo test` 跑。OpenPGP 插件的测试组织（[plugins/openpgp/tests/](file:///workspace/plugins/openpgp/tests/)）：

- [key_tests.rs](file:///workspace/plugins/openpgp/tests/key_tests.rs)：`key.generate`/`import`/`export`/`list`/`delete`。
- [encrypt_decrypt_tests.rs](file:///workspace/plugins/openpgp/tests/encrypt_decrypt_tests.rs)：`encrypt`/`decrypt`/`sym.encrypt`/`sym.decrypt`。
- [sign_verify_tests.rs](file:///workspace/plugins/openpgp/tests/sign_verify_tests.rs)：`sign`/`verify` 内联 + 分离。
- [armor_tests.rs](file:///workspace/plugins/openpgp/tests/armor_tests.rs)：`armor`/`dearmor` 往返。
- [keyring_tests.rs](file:///workspace/plugins/openpgp/tests/keyring_tests.rs)：钥匙串 CRUD。
- [gpg_tests.rs](file:///workspace/plugins/openpgp/tests/gpg_tests.rs)：`gpg.*` 命令族（CI 容器需装 gnupg）。

每个测试用 `tempfile::tempdir()` 创建隔离的 credentials_dir，避免污染真实 `~/.local/share/dyyl/credentials.d/`。模板（取自 [key_tests.rs#L17-L26](file:///workspace/plugins/openpgp/tests/key_tests.rs#L17-L26)）：

```rust
#![allow(clippy::unwrap_used)]  // 测试代码允许 unwrap

use openpgp::codec::DyylValue;
use openpgp::commands;
use openpgp::state::PluginState;
use std::path::PathBuf;
use tempfile::tempdir;

fn make_state() -> (PluginState, tempfile::TempDir) {
    let dir = tempdir().expect("create tempdir");
    let state = PluginState {
        credentials_dir: PathBuf::from(dir.path()),
        default_passphrase: Some("test-pass".to_string()),
        ..Default::default()
    };
    (state, dir)  // TempDir 必须保活到测试结束
}

#[test]
fn key_generate_returns_fingerprint_and_writes_files() {
    let (mut state, _dir) = make_state();
    let result = commands::dispatch(
        &mut state,
        "key.generate",
        &[DyylValue::Str("test <t@x>".into()), DyylValue::Str("pass123".into())],
    );
    let fp = result.expect("generate should succeed").as_str().unwrap().to_string();
    assert_eq!(fp.len(), 40, "fingerprint should be 40 hex chars");
    // ... 进一步断言文件存在、index.json 更新等
}
```

要点：

- `#![allow(clippy::unwrap_used)]` 在测试文件顶部放行 `unwrap_used` lint（测试代码允许 panic）。
- `tempdir` 返回的 `TempDir` 必须保活到测试结束，否则目录会被 drop 删除。
- 直接调 `commands::dispatch` 而非走完整 dlopen 路径，加快测试速度。
- gpg 测试用 `tempdir` + `GNUPGHOME` 环境变量隔离 gpg 钥匙串。

### 10.2 dyyl 主仓集成测试（dlopen fixture）

dyyl 主仓的 [tests/openpgp_plugin_tests.rs](file:///workspace/tests/openpgp_plugin_tests.rs) 用 `libloading` 真实 dlopen 加载插件产物，验证 15 符号全解析 + ABI 协议。

构建脚本 [tests/fixtures/build-openpgp.sh](file:///workspace/tests/fixtures/build-openpgp.sh) 在测试前 `cargo build --release`，把产物拷到测试 tempdir：

```bash
./tests/fixtures/build-openpgp.sh /tmp/openpgp-test
# 产物：/tmp/openpgp-test/libopenpgp.so
```

集成测试模板（取自 [openpgp_plugin_tests.rs#L75-L130](file:///workspace/tests/openpgp_plugin_tests.rs#L75-L130)）：

```rust
use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};

#[test]
fn test_abi_load_and_resolve_all_symbols() {
    let lib_path = build_plugin();  // 调 build-openpgp.sh
    let lib = unsafe { Library::new(&lib_path) }.expect("dlopen plugin");

    unsafe {
        // 解析全部 15 符号
        let _: Symbol<unsafe extern "C" fn() -> c_uint> =
            lib.get(b"dyyl_plugin_get_api_version").expect("get_api_version");
        let _: Symbol<unsafe extern "C" fn(c_uint) -> *mut c_void> =
            lib.get(b"dyyl_plugin_init").expect("init");
        // ... 其余 13 个符号同上
    }
}
```

### 10.3 e2e golden 脚本

dyyl 主仓的 [tests/openpgp_e2e_tests.rs](file:///workspace/tests/openpgp_e2e_tests.rs) 与 golden fixture 脚本配套，跑端到端流程：

- [openpgp-roundtrip.dyyl](file:///workspace/tests/fixtures/openpgp-roundtrip.dyyl)：generate → encrypt → decrypt → 验证原文一致。
- [openpgp-sign-verify.dyyl](file:///workspace/tests/fixtures/openpgp-sign-verify.dyyl)：generate → sign → verify → 验证 `valid:"1"`。
- [openpgp-sym.dyyl](file:///workspace/tests/fixtures/openpgp-sym.dyyl)：sym.encrypt → sym.decrypt 往返。
- [openpgp-gpg-detect.dyyl](file:///workspace/tests/fixtures/openpgp-gpg-detect.dyyl)：调 `gpg.detect` 输出 installed 字段。
- [openpgp-keyring-persist.dyyl](file:///workspace/tests/fixtures/openpgp-keyring-persist.dyyl)：跨脚本持久化（第一次脚本 generate 入库，第二次脚本 list 能看到）。

e2e 测试用 `tempdir` 设 `HOME` 环境变量隔离 XDG 目录，避免污染真实 `~/.local/share/dyyl/`。

### 10.4 CI 集成

CI 流程（设计文档 §7.6）：

1. `cargo build --release`（主 dyyl）。
2. `cd plugins/openpgp && cargo build --release && cargo test`（OpenPGP 插件单元测试）。
3. 拷产物到 dist + 运行 `scripts/publish-plugin.sh plugins/openpgp` 生成 manifest。
4. 跑 dyyl 主仓的 `tests/openpgp_plugin_tests.rs`（dlopen 集成）与 `tests/openpgp_e2e_tests.rs`（端到端）。

Linux CI 容器装 `gnupg` 包以支持 `gpg.*` 测试；macOS/Windows CI 仅跑 sequoia 命令族，gpg.* 用 `#[cfg(target_os = "linux")]` 或 `#[ignore]` 属性 gated（设计文档 §8.3）。

### 10.5 lint 合规（clippy deny 规则）

dyyl 主仓与 OpenPGP 插件 crate 的 `[lints.clippy]` 都设 `unwrap_used`/`panic`/`todo`/`unimplemented`/`indexing_slicing` 为 deny（[Cargo.toml#L32-L38](file:///workspace/plugins/openpgp/Cargo.toml#L32-L38)）。CI 跑：

```bash
cargo clippy --all-targets --all-features --workspace
cargo fmt --check
```

任意 clippy warning 视为 CI 失败。绕开策略：

- `unwrap()` → `unwrap_or(...)` / `unwrap_or_else(|| ...)` / `?`。
- `expect()` → 同上。
- `vec[i]` → `vec.get(i).ok_or_else(|| ...)?`。
- `panic!()` → 返回 `Result<_, PluginError>` 并用 `PluginError::runtime(msg)`。
- 测试代码顶部 `#![allow(clippy::unwrap_used)]` 放行。

OpenPGP 插件的 [codec.rs#L166-L169](file:///workspace/plugins/openpgp/src/codec.rs#L166-L169) 与 [lib.rs#L243-L246](file:///workspace/plugins/openpgp/src/lib.rs#L243-L246) 给了 `CString::new` 在不可失败场景下绕开 `unwrap` 的范例：先过滤 NUL 字节，再用 `unwrap_or_else(|_| empty_cstring())` 兜底（分支不可达，仅为类型系统）。

---

## 11. 完整范例：OpenPGP 插件

本章逐段讲解 [plugins/openpgp/](file:///workspace/plugins/openpgp/) 的实现。代码版本对应 v0.1.0，30 条命令分两族：17 条 sequoia 实现的 `openpgp.*` 与 13 条系统 gpg 包装的 `openpgp.gpg.*`。

### 11.1 设计目标与命令清单

设计目标（详见 [openpgp-plugin-and-guide-design.md §1.1](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)）：

- 用 `sequoia-openpgp`（纯 Rust，无系统依赖）实现核心 OpenPGP 操作。
- 默认密码套件现代：Ed25519 主密钥 + Curve25519 加密子密钥 + AES-256 + AEAD。
- 输出 ASCII armor。
- 同时提供独立的系统 gpg 集成命令族 `gpg.*`，与 sequoia 命令族并列、互不混用、不共享状态。
- 扩展 dyyl credentials 系统支持大型/动态凭据（file/directory 类型）。
- 30 条命令，全清单见 [command_list.json](file:///workspace/plugins/openpgp/command_list.json)。

sequoia 命令族（17 条）：

| 命令 | arity | 返回 | 失败哨兵 |
|---|---|---|---|
| `key.generate` | 2 | 指纹字符串 | `""` |
| `key.import` | 1 | 指纹字符串 | `""` |
| `key.export` | 2 | armored 字符串 | `""` |
| `key.list` | 0 | list of dict | `[]` |
| `key.delete` | 1 | `"1"`/`"0"` | `"0"` |
| `encrypt` | ≥2 | armored 密文 | `""` |
| `encrypt.file` | ≥3 | `"1"`/`"0"` | `"0"` |
| `decrypt` | 1+ | 明文 | `""` |
| `decrypt.file` | 2+ | `"1"`/`"0"` | `"0"` |
| `sign` | 2+ | armored 签名 | `""` |
| `sign.file` | 3+ | `"1"`/`"0"` | `"0"` |
| `verify` | 1-2 | dict `{valid, signer_uid, signer_fp}` | `{"valid":"0"}` |
| `verify.file` | 1-2 | 同上 dict | `{"valid":"0"}` |
| `sym.encrypt` | 2+ | armored 密文 | `""` |
| `sym.decrypt` | 2+ | 明文 | `""` |
| `armor` | 1 | armor 字符串 | `""` |
| `dearmor` | 1 | b64 字符串 | `""` |

gpg 命令族（13 条）：`gpg.detect`、`gpg.exec`、`gpg.encrypt`、`gpg.encrypt.file`、`gpg.decrypt`、`gpg.decrypt.file`、`gpg.sign`、`gpg.sign.file`、`gpg.verify`、`gpg.verify.file`、`gpg.key.list`、`gpg.key.import`、`gpg.key.export`。

### 11.2 crate 结构

```text
plugins/openpgp/
  Cargo.toml              # crate-type = ["cdylib", "rlib"], panic = "abort"
  plugin.toml.in          # manifest 模板（发布脚本填 sha256/url）
  command_list.json       # 30 条命令的 JSON 数组（list_commands 直接返回）
  src/
    lib.rs                # 15 个 ABI 符号导出 + handle_command 总入口
    state.rs              # PluginState：handle 持有的运行时状态
    creds.rs              # credentials JSON 解析（apply_credentials）
    keyring.rs            # 钥匙串 CRUD（读写 keys/*.asc + index.json）
    codec.rs              # DyylValue 枚举 + 编解码（与 dyyl value_codec 对称）
    error.rs              # PluginError + write_error
    commands/
      mod.rs              # dispatch match（30 分支）
      key.rs              # key.generate / import / export / list / delete
      encrypt.rs          # encrypt / encrypt.file / sym.encrypt
      decrypt.rs          # decrypt / decrypt.file / sym.decrypt
      sign.rs             # sign / sign.file
      verify.rs           # verify / verify.file
      armor.rs            # armor / dearmor
      gpg.rs              # gpg.* 全族
  tests/                  # 插件内单元测试
    key_tests.rs
    encrypt_decrypt_tests.rs
    sign_verify_tests.rs
    armor_tests.rs
    keyring_tests.rs
    gpg_tests.rs
```

### 11.3 PluginState 设计（handle 持有状态 + Mutex）

[state.rs](file:///workspace/plugins/openpgp/src/state.rs) 定义 handle 持有的状态：

```rust
pub struct PluginState {
    pub default_passphrase: Option<String>,   // 来自 credentials.toml [plugin.openpgp].passphrase
    pub default_key: Option<String>,          // 来自 [plugin.openpgp].default_key
    pub credentials_dir: PathBuf,             // credentials.d/openpgp/ 绝对路径
    pub key_cache: Mutex<HashMap<String, String>>,  // 已解锁私钥缓存（按指纹）
    pub index: Mutex<Option<KeyringIndex>>,   // keyring 索引懒加载缓存
}
```

设计要点：

- `default_passphrase` / `default_key` 由 `apply_credentials`（[creds.rs#L23-L46](file:///workspace/plugins/openpgp/src/creds.rs#L23-L46)）从 credentials JSON 填充。空字符串不写入（保持 None）。
- `credentials_dir` 是 `__credentials_dir` 注入的绝对路径，所有钥匙串文件读写都基于此路径。
- `key_cache` 用 `Mutex` 保护，同进程内复用已解锁私钥，避免反复 passphrase 解锁（实际实现里目前缓存的是指纹→序列化 armored 字符串的映射，详见 §11.9）。
- `index` 用 `Mutex<Option<KeyringIndex>>` 懒加载，首次 `key.list`/`encrypt` 等命令才读 `index.json`。`on_unload` 调 `clear_cache`（[state.rs#L43-L49](file:///workspace/plugins/openpgp/src/state.rs#L43-L49)）清空两个缓存。

`KeyringEntry` 与 `KeyringIndex` 也在 [state.rs#L6-L18](file:///workspace/plugins/openpgp/src/state.rs#L6-L18) 定义，对应 `index.json` 的 JSON 结构：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyringEntry {
    pub fp: String,
    pub uid: String,
    pub has_secret: bool,
    pub created: String,  // ISO-8601 UTC
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct KeyringIndex {
    pub keys: Vec<KeyringEntry>,
}
```

`init` 分配 `Box::new(PluginState::default())` 返回裸指针（[lib.rs#L60-L64](file:///workspace/plugins/openpgp/src/lib.rs#L60-L64)）；`shutdown` 用 `Box::from_raw` 回收（[lib.rs#L175-L184](file:///workspace/plugins/openpgp/src/lib.rs#L175-L184)）。

### 11.4 handle_command 分发

[lib.rs#L99-L144](file:///workspace/plugins/openpgp/src/lib.rs#L99-L144) 是 `handle_command` 的总入口：

```rust
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

    let cmd_str = unsafe { CStr::from_ptr(cmd) }.to_str().unwrap_or("");
    let args_str = unsafe { CStr::from_ptr(args) }.to_str().unwrap_or("[]");

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
```

关键步骤：

1. NULL 检查 + 把 `handle` 转回 `&mut PluginState`。
2. `CStr::from_ptr` 把 C 字符串转 Rust `&str`，用 `unwrap_or("")` 兜底非法 UTF-8。
3. `codec::decode_args` 解析 args JSON 数组为 `Vec<DyylValue>`。失败时写 `parse_failed` 错误对象，返回 1。
4. `commands::dispatch(state, cmd, &args)` 路由到具体子命令。成功时 `codec::encode_out` 把结果编码并写到 out，返回 0；失败时写错误对象，返回 1。

[commands/mod.rs](file:///workspace/plugins/openpgp/src/commands/mod.rs) 的 `dispatch` 是 30 分支的 `match cmd`：

```rust
pub fn dispatch(
    state: &mut PluginState,
    cmd: &str,
    args: &[DyylValue],
) -> Result<DyylValue, PluginError> {
    match cmd {
        "key.generate" => key::generate(state, args),
        "key.import" => key::import(state, args),
        // ... 其余 28 条
        "gpg.key.export" => gpg::key_export(state, args),
        _ => Err(PluginError::unknown_command(cmd)),
    }
}
```

子命令函数签名统一 `fn(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError>`。

### 11.5 key.generate 实现逐段讲解

[key.rs#L20-L77](file:///workspace/plugins/openpgp/src/commands/key.rs#L20-L77) 是 `key.generate` 的实现。设计目标：生成 Ed25519 主+Curve25519 加密子密钥，passphrase 加密私钥，写入钥匙串，返回指纹。

**第一步：解析参数 + passphrase 优先级**

```rust
let user_id = args
    .first()
    .and_then(DyylValue::as_str)
    .ok_or_else(|| PluginError::arity_mismatch("key.generate expects (user_id, passphrase)"))?;
let passphrase_arg = args.get(1).and_then(DyylValue::as_str).unwrap_or("_");

let passphrase: Option<String> = if passphrase_arg == "_" || passphrase_arg.is_empty() {
    state.default_passphrase.clone()
} else {
    Some(passphrase_arg.to_string())
};
```

`args.first()` 取第一个参数，`and_then(DyylValue::as_str)` 在非 Str 时返回 None，`ok_or_else` 转 `PluginError::arity_mismatch`。passphrase 解析遵循优先级：显式参数（非 `_` 非空）→ `state.default_passphrase` → 无 passphrase（生成无加密私钥）。

**第二步：用 sequoia CertBuilder 生成密钥**

```rust
use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::crypto::Password;

let (cert, _revocation) = CertBuilder::new()
    .add_userid(user_id)
    .set_cipher_suite(CipherSuite::Cv25519)   // Ed25519 + Curve25519
    .add_signing_subkey()
    .add_storage_encryption_subkey()
    .set_password(passphrase.as_ref().map(|p| Password::from(p.as_bytes())))
    .generate()
    .map_err(|e| PluginError::runtime(format!("key generation failed: {e}")))?;
```

`CipherSuite::Cv25519` 选现代密码套件。`add_signing_subkey` 加签名子密钥，`add_storage_encryption_subkey` 加存储加密子密钥。`set_password` 给私钥加 passphrase（无 passphrase 时传 None）。`generate()` 返回 `(Cert, RevocationCertificate)`。

**第三步：序列化为 armored 字符串**

```rust
let fp = cert.fingerprint().to_hex().to_uppercase();

let pub_armored = serialize_cert_to_armor(&cert, false)
    .map_err(|e| PluginError::runtime(format!("serialize public key: {e}")))?;
let sec_armored = serialize_cert_to_armor(&cert, true)
    .map_err(|e| PluginError::runtime(format!("serialize secret key: {e}")))?;
```

`serialize_cert_to_armor`（[key.rs#L207-L219](file:///workspace/plugins/openpgp/src/commands/key.rs#L207-L219)）用 sequoia 的 `SerializeInto` trait，私钥形式调 `cert.as_tsk().armored().to_vec()`，公钥形式调 `cert.armored().to_vec()`。指纹转大写 hex 字符串。

**第四步：写入钥匙串 + 更新 index**

```rust
let keyring = Keyring::new(state.credentials_dir.clone());
keyring.write_key_file(&fp, false, &pub_armored)
    .map_err(|e| PluginError::runtime(format!("write pub key: {e}")))?;
keyring.write_key_file(&fp, true, &sec_armored)
    .map_err(|e| PluginError::runtime(format!("write sec key: {e}")))?;

let uid = cert.userids().next().map(|u| u.userid().to_string()).unwrap_or_default();
let created = format_created(cert.primary_key().key().creation_time());

keyring.upsert_entry(KeyringEntry {
    fp: fp.clone(),
    uid,
    has_secret: true,
    created,
}).map_err(|e| PluginError::runtime(format!("update index: {e}")))?;

Ok(DyylValue::Str(fp))
```

`Keyring::write_key_file`（[keyring.rs#L85-L100](file:///workspace/plugins/openpgp/src/keyring.rs#L85-L100)）写文件到 `keys/<fp>.{pub,sec}.asc`，私钥文件 chmod 0600。`upsert_entry`（[keyring.rs#L54-L62](file:///workspace/plugins/openpgp/src/keyring.rs#L54-L62)）按指纹覆盖或追加到 `index.json`。最后返回指纹字符串。

### 11.6 encrypt/decrypt 实现

**encrypt**（[encrypt.rs#L83-L102](file:///workspace/plugins/openpgp/src/commands/encrypt.rs#L83-L102)）：

```rust
pub fn encrypt(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args.first().and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("encrypt expects (text, recipient, ...)"))?;

    let recipient_strs: Vec<&str> = args.iter().skip(1).filter_map(DyylValue::as_str).collect();
    if recipient_strs.is_empty() {
        return Err(PluginError::arity_mismatch("encrypt requires at least one recipient"));
    }

    let armored = encrypt_to_armored(state, text, &recipient_strs)?;
    Ok(DyylValue::Str(armored))
}
```

`encrypt_to_armored`（[encrypt.rs#L38-L79](file:///workspace/plugins/openpgp/src/commands/encrypt.rs#L38-L79)）是核心：

1. `resolve_recipient`（[encrypt.rs#L20-L34](file:///workspace/plugins/openpgp/src/commands/encrypt.rs#L20-L34)）：若 recipient 是 40 字符 hex 指纹，从钥匙串读公钥；否则当 armored 公钥解析。
2. 构造 sequoia `Message` 流水线：`Message::new(sink)` → `Armorer::new(message).build()` → `Encryptor::for_recipients(message, recipients).build()` → `LiteralWriter::new(message).build()`。
3. `writer.write_all(text.as_bytes())` 写明文。
4. `writer.finalize()` 完成。
5. 把 `sink: Vec<u8>` 转 String 返回。

`Encryptor::for_recipients` 接受多收件人迭代器，每个收件人的公钥都用 `cert.keys().with_policy(POLICY, None).supported().alive().revoked(false).for_storage_encryption()` 过滤出可用加密子密钥。

**decrypt**（[decrypt.rs#L159-L186](file:///workspace/plugins/openpgp/src/commands/decrypt.rs#L159-L186)）：

```rust
pub fn decrypt(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let armor = args.first().and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("decrypt expects (armor, passphrase?)"))?;
    let passphrase = resolve_passphrase(state, args.get(1).and_then(DyylValue::as_str))?;

    let secret_certs = load_secret_certs(state)?;  // 加载所有私钥
    let helper = DecryptHelper {
        certs: secret_certs,
        passphrase: Password::from(passphrase.as_bytes()),
    };

    let reader = std::io::Cursor::new(armor.as_bytes());
    let mut decryptor = DecryptorBuilder::from_reader(reader)
        .map_err(|e| PluginError::parse_failed(format!("build decryptor: {e}")))?
        .with_policy(POLICY, None, helper)
        .map_err(map_decrypt_error)?;

    let mut plaintext = Vec::new();
    decryptor.read_to_end(&mut plaintext)
        .map_err(|e| PluginError::runtime(format!("read plaintext: {e}")))?;

    let text = String::from_utf8(plaintext)
        .map_err(|e| PluginError::runtime(format!("plaintext not utf8: {e}")))?;
    Ok(DyylValue::Str(text))
}
```

`DecryptHelper`（[decrypt.rs#L69-L141](file:///workspace/plugins/openpgp/src/commands/decrypt.rs#L69-L141)）实现 sequoia 的 `DecryptionHelper` + `VerificationHelper` trait：

- `decrypt` 方法先试 SKESK（对称加密，用 passphrase 解），再试 PKESK（非对称，用每个私钥的加密子密钥解）。
- 私钥解密流程：`key.parts_into_secret()` → `decrypt_secret(&password)` → `into_keypair()` → `pkesk.decrypt(&mut keypair, sym_algo)`。
- 任意一步成功就调 `decrypt(algo, &sk)` 回调把会话密钥交给 sequoia，解密继续。
- 全失败返回 sequoia `Error::InvalidArgument`，`map_decrypt_error`（[decrypt.rs#L144-L155](file:///workspace/plugins/openpgp/src/commands/decrypt.rs#L144-L155)）按错误消息分类：含 "passphrase"/"No key to decrypt" 转 `passphrase_wrong`，否则转 `runtime`。

`sym.encrypt` / `sym.decrypt` 复用同样的 encrypt/decrypt 流水线，只是不传收件人证书、用 `Encryptor::with_passwords(message, Some(passphrase))`（[encrypt.rs#L161-L163](file:///workspace/plugins/openpgp/src/commands/encrypt.rs#L161-L163)）。

### 11.7 sign/verify 实现

**sign**（[sign.rs#L78-L140](file:///workspace/plugins/openpgp/src/commands/sign.rs#L78-L140)）：

```rust
pub fn sign(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text = args.first().and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign expects (text, key_fp, detach?, passphrase?)"))?;
    let fp = args.get(1).and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("sign expects (text, key_fp)"))?;
    let detach = args.get(2).and_then(DyylValue::as_str).map(|s| s == "1").unwrap_or(false);
    let passphrase = resolve_passphrase(state, args.get(3).and_then(DyylValue::as_str))?;

    let signer = load_signer(state, fp, &passphrase)?;

    let mut sink = Vec::new();
    if detach {
        // 分离签名：armor (Kind::Signature) → Signer (detached)
        let message = Message::new(&mut sink);
        let message = Armorer::new(message).kind(armor::Kind::Signature).build()
            .map_err(|e| PluginError::runtime(format!("build armorer: {e}")))?;
        let mut writer = Signer::new(message, signer)
            .map_err(|e| PluginError::runtime(format!("create signer: {e}")))?
            .detached().build()
            .map_err(|e| PluginError::runtime(format!("build detached signer: {e}")))?;
        writer.write_all(text.as_bytes())...;
        writer.finalize()...;
    } else {
        // 内联签名：armor (Kind::Message) → Signer → LiteralWriter
        let message = Message::new(&mut sink);
        let message = Armorer::new(message).build()...;
        let message = Signer::new(message, signer).build()...;
        let mut writer = LiteralWriter::new(message).build()...;
        writer.write_all(text.as_bytes())...;
        writer.finalize()...;
    }

    let armored = String::from_utf8(sink)...;
    Ok(DyylValue::Str(armored))
}
```

`load_signer`（[sign.rs#L21-L55](file:///workspace/plugins/openpgp/src/commands/sign.rs#L21-L55)）：从钥匙串读 `<fp>.sec.asc` → `Cert::from_str` → `cert.keys().secret().for_signing().next()` 找签名子密钥 → `parts_into_secret()` → `decrypt_secret(&password)` → `into_keypair()`。passphrase 错误时 `decrypt_secret` 失败，转 `PluginError::passphrase_wrong`。

**verify**（[verify.rs#L171-L190](file:///workspace/plugins/openpgp/src/commands/verify.rs#L171-L190)）：

```rust
pub fn verify(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let text_or_sig = args.first().and_then(DyylValue::as_str)
        .ok_or_else(|| PluginError::arity_mismatch("verify expects (text_or_sig, signed_text?)"))?;
    let signed_text = args.get(1).and_then(DyylValue::as_str);

    let certs = load_pub_certs(state).unwrap_or_default();
    let helper = VerifyHelper::new(certs);

    let helper = if let Some(data) = signed_text {
        verify_detached(text_or_sig, data, helper)
    } else {
        verify_inline(text_or_sig, helper)
    };

    Ok(make_result(helper.valid, &helper.signer_uid, &helper.signer_fp))
}
```

`VerifyHelper`（[verify.rs#L24-L89](file:///workspace/plugins/openpgp/src/commands/verify.rs#L24-L89)）实现 `VerificationHelper`：

- `get_certs`：按 sequoia 请求的 KeyHandle 过滤可用证书，无匹配时 fallback 提供所有证书。
- `check`：遍历 `MessageStructure`，对 `SignatureGroup` 层取 `results.into_iter().flatten()` 的 `GoodSignature`，记录 `cert.fingerprint()` 与首个 userid。

`verify` 返回 dict `{valid, signer_uid, signer_fp}`（[verify.rs#L110-L125](file:///workspace/plugins/openpgp/src/commands/verify.rs#L110-L125) 的 `make_result`）。验签失败时不返回 `Err`，而是返回 `{valid:"0", ...}`，让脚本通过 `valid` 字段判断。

### 11.8 gpg.* 命令族实现（系统 gpg 集成）

[gpg.rs](file:///workspace/plugins/openpgp/src/commands/gpg.rs) 是 13 条 gpg 包装命令。设计约束（[设计文档 §3.4](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)）：完全不读 `PluginState` 的 credentials/keyring，纯系统调用。

核心是 `run_gpg`（[gpg.rs#L26-L60](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L26-L60)）：

```rust
fn run_gpg(args: &[&str], stdin: Option<&[u8]>) -> Result<(String, String, i32), PluginError> {
    let gpg = gpg_path().ok_or_else(|| {
        PluginError::gpg_not_installed("gpg binary not found in PATH")
    })?;

    let mut cmd = Command::new(&gpg);
    cmd.args(args);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|e| PluginError::gpg_exec_failed(format!("spawn gpg: {e}")))?;

    if let Some(data) = stdin {
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin.write_all(data)
                .map_err(|e| PluginError::gpg_exec_failed(format!("write stdin: {e}")))?;
        }
    }
    drop(child.stdin.take());  // EOF

    let output = child.wait_with_output()
        .map_err(|e| PluginError::gpg_exec_failed(format!("wait gpg: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, code))
}
```

要点：

- `gpg_path()` 用 `which::which("gpg")` 找 gpg 二进制路径。
- `stdin` 参数支持把数据 pipe 给 gpg（`gpg.encrypt` 把明文 pipe 进 stdin）。
- 返回 stdout、stderr、退出码三元组。退出码非 0 时上层调 `run_gpg_or_fail` 转 `PluginError::gpg_exec_failed`。

`gpg.detect`（[gpg.rs#L95-L119](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L95-L119)）永远不报错：找不到 gpg 返回 `{installed:"0", path:"", version:""}`；找到则调 `gpg --version` 取首行版本号。

`gpg.exec`（[gpg.rs#L123-L146](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L123-L146)）支持两种 args 形式：Str 时用 `shell_words::split` 按 shell 风格拆分，List 时每个元素当一个参数。失败时返回空字符串 + stderr 走 `eprintln!`（`--debug` 时可见）。

高层包装（`gpg.encrypt`、`gpg.sign` 等）内部组装 gpg 参数 + 调 `run_gpg_or_fail`，参数构造见 [gpg.rs#L149-L460](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L149-L460)。

`gpg.verify`（[gpg.rs#L291-L327](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L291-L327)）分离验签时把 sig 与 data 写到 `tempfile::NamedTempFile`，再调 `gpg --verify <sig> <data>`。从 stderr 用 `extract_signer_from_gpg_output`（[gpg.rs#L80-L91](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L80-L91)）正则提取 `Good signature from "Name"` 行的签名者。

`gpg.key.list`（[gpg.rs#L354-L401](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L354-L401)）调 `gpg --list-keys --with-colons`，按冒号分隔字段解析 `pub`/`sec`/`fpr`/`uid` 行。

### 11.9 credentials.d 钥匙串管理

[keyring.rs](file:///workspace/plugins/openpgp/src/keyring.rs) 实现钥匙串 CRUD。文件布局（设计文档 §5.4）：

```text
credentials.d/openpgp/
  keys/
    ABCD1234EF567890.pub.asc      # 公钥
    ABCD1234EF567890.sec.asc      # 私钥（passphrase 加密）
  index.json                       # KeyringIndex 序列化
```

`Keyring` 结构（[keyring.rs#L8-L11](file:///workspace/plugins/openpgp/src/keyring.rs#L8-L11)）持有 `base_dir: PathBuf`（即 `state.credentials_dir`）。

关键操作：

- `load_index`（[keyring.rs#L32-L39](file:///workspace/plugins/openpgp/src/keyring.rs#L32-L39)）：读 `index.json` 反序列化为 `KeyringIndex`。文件不存在返回空默认值（fresh keyring）。
- `save_index`（[keyring.rs#L42-L49](file:///workspace/plugins/openpgp/src/keyring.rs#L42-L49)）：pretty-printed JSON 写 `index.json`。
- `upsert_entry`（[keyring.rs#L54-L62](file:///workspace/plugins/openpgp/src/keyring.rs#L54-L62)）：按指纹覆盖或追加。
- `remove_entry`（[keyring.rs#L67-L81](file:///workspace/plugins/openpgp/src/keyring.rs#L67-L81)）：从 index 删 + 删 `keys/<fp>.{pub,sec}.asc` 文件。幂等。
- `write_key_file`（[keyring.rs#L85-L100](file:///workspace/plugins/openpgp/src/keyring.rs#L85-L100)）：写文件，私钥 chmod 0600（Unix）。
- `read_key_file`（[keyring.rs#L103-L110](file:///workspace/plugins/openpgp/src/keyring.rs#L103-L110)）：读文件，不存在返 Err。

`key.list` 直接读 `index.json`（[key.rs#L147-L180](file:///workspace/plugins/openpgp/src/commands/key.rs#L147-L180)），避免扫目录 + 解析 armored：

```rust
pub fn list(state: &mut PluginState, _args: &[DyylValue]) -> Result<DyylValue, PluginError> {
    let keyring = Keyring::new(state.credentials_dir.clone());
    let index = keyring.load_index()
        .map_err(|e| PluginError::runtime(format!("load index: {e}")))?;

    let list: Vec<DyylValue> = index.keys.iter().map(|entry| {
        DyylValue::Dict(vec![
            (DyylValue::Str("fp".into()), DyylValue::Str(entry.fp.clone())),
            (DyylValue::Str("uid".into()), DyylValue::Str(entry.uid.clone())),
            (DyylValue::Str("secret".into()), DyylValue::Str(if entry.has_secret { "1" } else { "0" }.into())),
            (DyylValue::Str("created".into()), DyylValue::Str(entry.created.clone())),
        ])
    }).collect();

    Ok(DyylValue::List(list))
}
```

私钥缓存 `state.key_cache: Mutex<HashMap<String, String>>` 用于在同一脚本内复用已解锁的私钥 armored 字符串（实际实现里缓存的是指纹→armored 字符串，避免反复读文件；passphrase 解锁仍是每次 `load_signer` 都做一遍——后续可优化为缓存 `KeyPair`，但 sequoia 的 `KeyPair` 不是 `Send`，跨线程缓存复杂）。

`on_unload` 调 `state.clear_cache()`（[state.rs#L43-L49](file:///workspace/plugins/openpgp/src/state.rs#L43-L49)）清空两个缓存，保证脚本结束后私钥数据从内存释放。

### 11.10 错误码实践

[error.rs](file:///workspace/plugins/openpgp/src/error.rs) 定义 `PluginError` 结构与 10 个错误码构造器：

| code | 构造器 | 触发场景 |
|---|---|---|
| `arity_mismatch` | `arity_mismatch(msg)` | 参数数量不符 |
| `type_error` | `type_error(msg)` | 参数类型不对（如 `gpg.exec` 收到非 Str/List） |
| `unknown_command` | `unknown_command(cmd)` | dispatch match 未命中 |
| `runtime` | `runtime(msg)` | 内部运行错误（IO 失败、sequoia API 失败等） |
| `key_not_found` | `key_not_found(msg)` | 指纹在钥匙串中不存在 |
| `passphrase_wrong` | `passphrase_wrong(msg)` | passphrase 无法解锁私钥/解密失败 |
| `parse_failed` | `parse_failed(msg)` | armored 输入解析失败 |
| `verify_failed` | `verify_failed(msg)` | 验签失败（签名无效） |
| `gpg_not_installed` | `gpg_not_installed(msg)` | `gpg.*` 调用但系统未装 gpg |
| `gpg_exec_failed` | `gpg_exec_failed(msg)` | gpg 二进制执行失败（退出码非 0） |

`write_error`（[error.rs#L105-L116](file:///workspace/plugins/openpgp/src/error.rs#L105-L116)）写 JSON 到 out：

```rust
pub fn write_error(out: *mut *mut c_char, code: &str, message: &str) {
    let json = format!(
        r#"{{"code":{},"message":{}}}"#,
        serde_json::to_string(code).unwrap_or_else(|_| "\"\"".to_string()),
        serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_string()),
    );
    let c = cstring_from_str(&json);
    unsafe { *out = c.into_raw(); }
}
```

用 `serde_json::to_string` 对 code/message 做 JSON 字符串转义，避免内嵌引号破坏 JSON。

`verify` 与 `gpg.verify` 不用 `verify_failed` 错误码：验签失败时返回 `Ok(Dict({valid:"0", ...}))`，让脚本通过 `valid` 字段判断，不触发 RuntimeError。这与"失败哨兵"约定一致（§11.1 表格）。

### 11.11 测试套件组织

OpenPGP 插件的测试分四层（设计文档 §8）：

**插件 crate 单元测试**（`plugins/openpgp/tests/`）：直接调 `commands::dispatch`，不走 dlopen。每个测试用 `tempfile::tempdir()` 隔离 credentials_dir。模板见 §10.1。

**dyyl credentials 扩展单元**（dyyl 主仓 `tests/plugin_credentials_inject_tests.rs` 与 `tests/credentials_tests.rs`）：覆盖 file/directory 类型注入、权限、缺失文件处理、`__credentials_dir` 自动注入。

**集成测试**（dyyl 主仓 [tests/openpgp_plugin_tests.rs](file:///workspace/tests/openpgp_plugin_tests.rs)）：真实 dlopen + 15 符号全解析 + 各命令通过 `handle_command` 调用。构建脚本 [tests/fixtures/build-openpgp.sh](file:///workspace/tests/fixtures/build-openpgp.sh) 在测试前编译插件。

**e2e golden**（dyyl 主仓 [tests/openpgp_e2e_tests.rs](file:///workspace/tests/openpgp_e2e_tests.rs) + [tests/fixtures/openpgp-*.dyyl](file:///workspace/tests/fixtures/)）：脚本里调 `openpgp.*` 完整流程，golden 输出对比。脚本例（[openpgp-roundtrip.dyyl](file:///workspace/tests/fixtures/openpgp-roundtrip.dyyl)）：

```dyyl
set $fp, openpgp.key.generate "test <test@example.com>", "test-pass"
io.out $fp
set $ct, openpgp.encrypt "secret message", $fp
io.out $ct
set $pt, openpgp.decrypt $ct, "test-pass"
io.out $pt
```

gpg 测试用 `tempdir` 设 `GNUPGHOME` 环境变量隔离 gpg 钥匙串，避免污染真实 `~/.gnupg`。

---

## 12. 已知风险与约束

### 12.1 信任模型：插件与脚本无限信任

dyyl 插件与脚本之间是**无限信任**关系：

- 插件与 dyyl 进程**同权限**，无沙箱、无能力隔离。
- 插件可以读写任意 dyyl 能访问的文件、起任意子进程、调任意系统调用。
- 插件与脚本之间不互相防御：脚本传给插件的参数不做校验，插件返回给脚本的值也不做校验。
- credentials.toml 明文存储（§12.3），插件能读到自己声明的所有凭证字段。

这是设计决策（详见 [plugin-ecosystem-design.md §1.2](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md) 与 [openpgp-plugin-and-guide-design.md §1.3](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)）：dyyl 不做防御性安全，shell 注入、路径注入等不在威胁模型内。用户应当只安装可信来源的插件（官方源 `l.dyyapp.com` 是默认且唯一支持的源）。

### 12.2 panic = "abort" 是硬性约束

跨 FFI 边界 panic 是 UB（§3.4）。`[profile.release] panic = "abort"` 把 panic 降级为进程终止，是硬性约束：

- dyyl 加载时不强制验证编译选项（只校验 manifest 的 `panic_mode` 字段值，不反编译产物）。
- 作者必须自行保证 Cargo.toml 配置正确。
- 配套的 `[lints.clippy]` deny `unwrap_used`/`panic`/`indexing_slicing` 等规则帮助避免运行时 panic。
- 这是技术约束（FFI UB），不是信任问题（与 §12.1 无关）。

### 12.3 credentials.toml 明文存储

`~/.config/dyyl/credentials.toml` 是明文 TOML 文件，存储 AI API key、插件 passphrase 等敏感凭证。dyyl 不加密、不强制 chmod 修正权限（`--debug` 时权限过松仅警告）。这是事实陈述，非威胁模型：用户对自己的 `~/.config` 目录权限负责。

`credentials.d/<plugin>/keys/*.sec.asc` 私钥文件由插件自己 chmod 0600（OpenPGP 插件在 [keyring.rs#L91-L98](file:///workspace/plugins/openpgp/src/keyring.rs#L91-L98)）。passphrase 加密的私钥即使文件被读也需 passphrase 才能解锁。

### 12.4 私钥内存管理 best practice

OpenPGP 插件的私钥内存管理：

- 私钥在内存中解密后用完即 drop（sequoia 的 `KeyPair` 在作用域结束自动 drop）。
- 同进程内的私钥缓存：`state.key_cache: Mutex<HashMap<String, String>>` 缓存已解锁私钥的 armored 字符串（不是 KeyPair，因 sequoia `KeyPair` 不是 `Send`）。
- `on_unload` 调 `state.clear_cache()` 清空缓存，保证脚本结束后私钥数据从内存释放。

这是 best practice，但**非安全边界**：

- 内存中的私钥数据在 drop 后只是被 Rust allocator 标记为可重用，不主动 zeroize（除非用 `zeroize` crate，OpenPGP 插件当前未引入）。
- swap 到磁盘的私钥数据不在控制范围内（用户应自行配置 swap 加密）。
- 其它进程可通过 ptrace、/proc/pid/mem 等机制读取 dyyl 进程内存（与 dyyl 同权限模型一致）。

### 12.5 gpg.* 命令族是系统调用包装

`openpgp.gpg.*` 命令族是系统 gpg 二进制的纯包装：

- 行为由 gpg 二进制版本决定（不同 GnuPG 版本 CLI 行为略有差异）。
- gpg 退出码非 0 时插件返回空字符串哨兵 + `--debug` 输出 stderr（[gpg.rs#L141-L145](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L141-L145)）。
- gpg.* 完全不读 `PluginState` 的 credentials/keyring，使用系统 `~/.gnupg` keyring（由 `GNUPGHOME` 环境变量控制）。
- 与 sequoia 命令族的钥匙串完全独立、不共享（设计约束，详见 [设计文档 §3.4](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md)）。

---

## 13. 故障排查

### 13.1 常见错误码与含义

dyyl 主仓 i18n 文案里的 `plugin.*` 键覆盖了大部分场景（[en.json#L67-L95](file:///workspace/locales/en.json#L67-L95)）：

| 错误消息（zh） | 触发场景 | 排查方向 |
|---|---|---|
| `插件 '<name>' ABI 版本不匹配: 期望 2, 实际 N` | manifest `abi_version` 不是 1 或 2 | 改 manifest 的 abi_version；重编译插件设正确版本。 |
| `插件 '<name>' 无 <platform> 构建; 可用: ...` | manifest.platforms 无当前平台条目 | 跨平台构建产物并加 platforms 条目（§9.3）。 |
| `插件 '<name>' SHA256 校验和不匹配` | 下载产物 sha256 与 manifest 声明不符 | 重新发布（产物变化后 sha256sum 重算并更新 manifest）。 |
| `加载插件 '<name>' 失败: missing symbol: dyyl_plugin_xxx` | 缺少某 ABI 符号 | 检查 `#[no_mangle] pub extern "C"` 是否齐全（15 个）。 |
| `加载插件 '<name>' 失败: init() returned NULL` | `dyyl_plugin_init` 返回 NULL | 检查 init 内部内存分配是否失败。 |
| `加载插件 '<name>' 失败: on_load() failed with code N` | `on_load` 返回非 0 | 检查 on_load 内部初始化逻辑。 |
| `加载插件 '<name>' 失败: set_credentials() failed with code N` | `set_credentials` 返回非 0 | 检查 `apply_credentials` 解析 JSON 是否失败。 |
| `插件 '<name>' 无命令 '<sub>'` | manifest commands 里找不到 sub | 加 manifest 条目或检查命令名拼写（含点号的多级名）。 |
| `插件命令 '<name>.<sub>' 失败: <code>` | `handle_command` 返回非 0 | 看 `<code>` 字段（插件自己的错误码，详见 §11.10）。 |

### 13.2 dlopen 失败

`dlopen failed for <name>: <e>` 错误的常见原因：

1. **缺符号**：`missing symbol: dyyl_plugin_xxx`。dyyl 期望 15 个符号全导出（ABI v2）。检查 `Cargo.toml` 的 `crate-type = ["cdylib"]` 与 `lib.name` 配置，确保 15 个 `#[no_mangle] pub extern "C"` 都在。
2. **ABI 不匹配**：`API version mismatch: plugin=N, dyyl supports 1 and 2`。改 `dyyl_plugin_get_api_version` 返回 1 或 2，或重编译插件对应正确版本。
3. **平台条目缺**：`plugin '<name>' has no build for <platform>`。manifest.platforms 数组里没当前 `<os>-<arch>` 条目，需跨平台构建并发布。
4. **文件路径错**：`dlopen failed: <path>: cannot open shared object file`。检查 `dist/plugins/<name>/<version>/<platform>/lib<name>.so` 是否存在、文件名是否与 `lib.name` 一致。

### 13.3 SHA256 不符

`plugin '<name>' SHA256 checksum mismatch` 错误：

- 重新发布：`./scripts/publish-plugin.sh plugins/<name>`，脚本会重算 sha256 并更新 manifest。
- 手动发布：`sha256sum dist/plugins/<name>/<version>/<platform>/<filename> | cut -d' ' -f1` 拿 hash，填到 manifest 的 `platforms[].sha256` 字段。
- 注意：跨平台发布时每个平台的产物 sha256 不同，要分别计算。

### 13.4 credentials 提示循环

`plugin '<name>' needs credentials, please enter:` 反复出现：

- 检查 `credentials.toml` 的 `[plugin.<name>]` 段是否正确写入（dyyl 提示后会自动保存，但若文件权限不允许写入会失败）。
- 检查 stdin 是否 EOF：非交互环境（CI、脚本 pipe）无法读 stdin，应预先写好 `credentials.toml` 再运行 dyyl。
- 检查 manifest 的 `credentials.fields[].name` 与 `credentials.toml` 字段名是否一致（区分大小写）。

### 13.5 --debug 输出解读

`dyyl --debug script.dyyl` 会输出额外诊断信息到 stderr：

- `i18n warning: zh translation missing for '<key>', falling back to en`：dyyl 主表某 zh 键缺失（CI 应当用 `missing_translations` 函数 [i18n.rs#L117-L128](file:///workspace/src/i18n.rs#L117-L128) 拦截）。
- `warning: credential file '<path>' for plugin '<name>' not found, injecting empty`：`type:"file"` 字段文件不存在（[creds_inject.rs#L64-L69](file:///workspace/src/runtime/plugin/creds_inject.rs#L64-L69)）。
- `[openpgp] gpg exited <code>: <stderr>`：`gpg.exec` 命令 gpg 退出码非 0（[gpg.rs#L142-L143](file:///workspace/plugins/openpgp/src/commands/gpg.rs#L142-L143)）。
- `RuntimeError` 详情：每条 RuntimeError 包含行号、命令文本、错误消息。

### 13.6 如何报告插件 bug

- dyyl 主仓 bug（ABI、credentials 注入、manifest 解析、i18n 路由等）：在 dyyl 主仓提 issue，附 `--debug` 输出 + 复现脚本 + 涉及的插件名/版本。
- OpenPGP 插件 bug（具体命令实现、错误码、sequoia 行为）：在 OpenPGP 插件 crate 提 issue，附命令调用 + 期望输出 + 实际输出 + 错误码。
- 系统 gpg 行为 bug（`gpg.*` 命令族）：先确认 `gpg` 二进制本身的版本与行为（`gpg --version`），再判断是否是插件包装层的问题。

报告时附上：

- dyyl 版本（`dyyl --version` 或 `target/release/dyyl` 的编译时间）。
- 插件名与版本（`~/.local/share/dyyl/plugins/<name>/<version>/plugin.toml` 的 version 字段）。
- 操作系统与架构（`uname -s` + `uname -m`）。
- 复现脚本（最小化的 `.dyyl` 文件）。
- `--debug` 输出（stderr）。

---

## 14. 参考资源

### 14.1 dyyl 主仓文档

- [README.md](file:///workspace/README.md) —— dyyl 项目概览、CLI 用法、语言基础。
- [dyyl-api-reference.md](file:///workspace/dyyl-api-reference.md) —— 脚本作者向 API 参考（语法、变量、命令族、返回值约定）。
- [docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md) —— 插件生态设计 spec（ABI 决策、manifest schema、CLI 子命令、i18n 重设计）。
- [docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md](file:///workspace/docs/superpowers/specs/2026-07-14-openpgp-plugin-and-guide-design.md) —— OpenPGP 插件与本指南的设计 spec（命令清单、credentials 扩展、钥匙串布局、测试策略）。
- [docs/superpowers/specs/2026-07-13-ai-credentials-logic-end-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-ai-credentials-logic-end-design.md) —— credentials 系统 ABI v2 设计（`set_credentials` 引入）。

### 14.2 dyyl 核心源码

- [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs) —— 15 个 ABI 符号名 + 函数指针类型 + `DYRL_API_VERSION` 常量。
- [src/runtime/plugin/mod.rs](file:///workspace/src/runtime/plugin/mod.rs) —— `PluginManager` 与 `dispatch`/`load_plugin`/`assemble_credentials`/`install_plugin`。
- [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs) —— `PluginLoader`：dlopen + 符号解析 + 调用顺序（init → set_credentials → on_load）。
- [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs) —— `RemoteManifest`/`LocalPluginToml`/`CredentialsSpec`/`CredentialField` 结构。
- [src/runtime/plugin/creds_inject.rs](file:///workspace/src/runtime/plugin/creds_inject.rs) —— `build_credentials_json`：按字段 type 分发（string/file/directory）。
- [src/runtime/plugin/value_codec.rs](file:///workspace/src/runtime/plugin/value_codec.rs) —— dyyl 侧 Value ↔ JSON 编解码。
- [src/runtime/plugin/store.rs](file:///workspace/src/runtime/plugin/store.rs) —— XDG 路径管理（`plugin_dir`/`lib_path`/`current_platform`）。
- [src/runtime/plugin/registry.rs](file:///workspace/src/runtime/plugin/registry.rs) —— 已装插件扫描。
- [src/credentials.rs](file:///workspace/src/credentials.rs) —— `CredentialsFile` 读写、`credentials_dir_for_plugin`、`ensure_plugin_credentials` 交互提示。
- [src/i18n.rs](file:///workspace/src/i18n.rs) —— `MessageStore`、`t()`、`register_plugin`、zh fallback 到 en。
- [src/runtime/cmd/plugin.rs](file:///workspace/src/runtime/cmd/plugin.rs) —— `dispatch_plugin_command`：脚本命令路由到 PluginManager。

### 14.3 OpenPGP 插件源码

- [plugins/openpgp/Cargo.toml](file:///workspace/plugins/openpgp/Cargo.toml) —— crate 配置（cdylib + rlib + panic=abort + clippy lints）。
- [plugins/openpgp/plugin.toml.in](file:///workspace/plugins/openpgp/plugin.toml.in) —— manifest 模板（30 命令 + 3 credentials 字段）。
- [plugins/openpgp/command_list.json](file:///workspace/plugins/openpgp/command_list.json) —— 30 条命令的 JSON 数组。
- [plugins/openpgp/src/lib.rs](file:///workspace/plugins/openpgp/src/lib.rs) —— 15 个 ABI 符号导出。
- [plugins/openpgp/src/state.rs](file:///workspace/plugins/openpgp/src/state.rs) —— `PluginState` + `KeyringIndex` + `KeyringEntry`。
- [plugins/openpgp/src/codec.rs](file:///workspace/plugins/openpgp/src/codec.rs) —— 插件侧 `DyylValue` 编解码。
- [plugins/openpgp/src/creds.rs](file:///workspace/plugins/openpgp/src/creds.rs) —— `apply_credentials`：JSON → PluginState。
- [plugins/openpgp/src/error.rs](file:///workspace/plugins/openpgp/src/error.rs) —— `PluginError` + 10 个错误码。
- [plugins/openpgp/src/keyring.rs](file:///workspace/plugins/openpgp/src/keyring.rs) —— 钥匙串 CRUD。
- [plugins/openpgp/src/commands/mod.rs](file:///workspace/plugins/openpgp/src/commands/mod.rs) —— 30 分支 dispatch。
- [plugins/openpgp/src/commands/key.rs](file:///workspace/plugins/openpgp/src/commands/key.rs) —— `key.generate`/`import`/`export`/`list`/`delete`。
- [plugins/openpgp/src/commands/encrypt.rs](file:///workspace/plugins/openpgp/src/commands/encrypt.rs) —— `encrypt`/`encrypt.file`/`sym.encrypt`。
- [plugins/openpgp/src/commands/decrypt.rs](file:///workspace/plugins/openpgp/src/commands/decrypt.rs) —— `decrypt`/`decrypt.file`/`sym.decrypt` + `DecryptHelper`。
- [plugins/openpgp/src/commands/sign.rs](file:///workspace/plugins/openpgp/src/commands/sign.rs) —— `sign`/`sign.file` + `load_signer`。
- [plugins/openpgp/src/commands/verify.rs](file:///workspace/plugins/openpgp/src/commands/verify.rs) —— `verify`/`verify.file` + `VerifyHelper`。
- [plugins/openpgp/src/commands/armor.rs](file:///workspace/plugins/openpgp/src/commands/armor.rs) —— `armor`/`dearmor`。
- [plugins/openpgp/src/commands/gpg.rs](file:///workspace/plugins/openpgp/src/commands/gpg.rs) —— 13 条 `gpg.*` 命令。

### 14.4 测试与发布

- [scripts/publish-plugin.sh](file:///workspace/scripts/publish-plugin.sh) —— 发布脚本（构建 + sha256 + manifest 生成）。
- [server.js](file:///workspace/server.js) —— 本地分发服务器（端口 8951，`/plugins/<name>/manifest.json` 与 `/plugins/<name>/<version>/<platform>/<filename>` 路由）。
- [tests/fixtures/example-plugin/](file:///workspace/tests/fixtures/example-plugin/) —— 最小插件示例（30 行级别）。
- [tests/fixtures/build-openpgp.sh](file:///workspace/tests/fixtures/build-openpgp.sh) —— OpenPGP 插件构建脚本（集成测试前调用）。
- [tests/openpgp_plugin_tests.rs](file:///workspace/tests/openpgp_plugin_tests.rs) —— dlopen 集成测试。
- [tests/openpgp_e2e_tests.rs](file:///workspace/tests/openpgp_e2e_tests.rs) —— e2e golden 脚本测试。
- [tests/fixtures/openpgp-roundtrip.dyyl](file:///workspace/tests/fixtures/openpgp-roundtrip.dyyl) —— generate → encrypt → decrypt golden。
- [tests/plugin_credentials_inject_tests.rs](file:///workspace/tests/plugin_credentials_inject_tests.rs) —— credentials file/directory 注入测试。

### 14.5 外部资源

- **sequoia-openpgp crate 文档**：<https://docs.rs/sequoia-openpgp> —— OpenPGP 插件依赖的核心库，提供 `Cert`/`CertBuilder`/`Message`/`Encryptor`/`Signer`/`DecryptorBuilder`/`VerifierBuilder` 等 API。
- **sequoia-openpgp 用户指南**：<https://sequoia-pgp.org/> —— sequoia 项目主页与教程。
- **RFC 4880**：OpenPGP 消息格式（已废止，被 RFC 9580 替代，但 sequoia 兼容）。
- **RFC 6637**：ECC 在 OpenPGP 中的使用（Ed25519/Curve25519 相关）。
- **RFC 9580**（2024）：OpenPGP 标准更新，取代 RFC 4880。
- **libloading crate 文档**：<https://docs.rs/libloading> —— dyyl 用的 dlopen 包装库。
- **which crate 文档**：<https://docs.rs/which> —— `gpg.detect` 用的 PATH 查找库。
- **shell-words crate 文档**：<https://docs.rs/shell-words> —— `gpg.exec` 用的 shell 风格参数拆分库。
