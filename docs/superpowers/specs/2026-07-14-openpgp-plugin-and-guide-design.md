# dyyl OpenPGP 插件与插件开发指南设计

**日期：** 2026-07-14
**状态：** 设计已批准，待写实现计划
**关联：**
- dyyl v0.2.0
- 基于 [2026-07-13-plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)（插件生态）
- 基于 [2026-07-13-ai-credentials-logic-end-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-ai-credentials-logic-end-design.md)（credentials 系统，ABI v2）

---

## 1. 目标与非目标

### 1.1 目标

- 在 dyyl 主仓 `plugins/openpgp/` 目录下实现一个生产级 OpenPGP 插件，使用 `sequoia-openpgp` crate（纯 Rust，无系统依赖）。
- 插件 v1 覆盖核心全套 OpenPGP 操作：密钥生成/导入/导出/列出/删除、非对称加解密、对称加解密、签名与验签（含分离签名）、armor/dearmor。
- 同时提供独立的系统 gpg 集成命令族 `openpgp.gpg.*`（detect + exec + 高层包装），与 sequoia 命令族并列、互不混用、不共享状态。
- 扩展 dyyl credentials 系统支持大型/动态凭据：新增 `type: "file"` 与 `type: "directory"` 字段类型，OpenPGP 用 `credentials.d/openpgp/keys/` 子目录承载动态密钥。
- 编写一份完整的插件开发指南 `docs/plugin-development-guide.md`，单文件综合，既是教程又是参考手册，以 OpenPGP 插件作为贯穿范例。

### 1.2 非目标（v1）

- 不实现 HKP 密钥服务器查询。
- 不实现信任模型（TOFU / Web of Trust）。
- 不实现智能卡 / 软令牌（sequoia-keystore 集成）。
- 不实现 per-key passphrase 持久化（所有密钥共用一个默认 passphrase，调用时可覆盖）。
- 不测 sequoia 与系统 GnuPG 二进制的互操作（v1 不强制）。
- 不实现 sequoia 命令族与 gpg 命令族之间的密钥共享（完全独立的两个钥匙串）。
- 不引入新的 ABI 版本（仍用 ABI v2 的 15 符号，`set_credentials` 签名不变；file/directory 是 manifest 层面的扩展）。

### 1.3 信任模型

与现有 spec §1.2 一致：**插件与脚本无限信任**，与 dyyl 进程同权限，无沙箱。本设计不讨论防御性安全措施（shell 注入、路径注入等不在威胁模型内），只在文档中客观记录已知风险（panic = abort 硬性约束、credentials.toml 明文存储等）。

---

## 2. 关键决策汇总

| # | 维度 | 决策 |
|---|---|---|
| 1 | OpenPGP 库 | `sequoia-openpgp`（纯 Rust，无系统依赖，干净编进 cdylib） |
| 2 | 命令范围（sequoia） | 核心全套：key.generate/import/export/list/delete + encrypt/decrypt + sym.encrypt/decrypt + sign/verify + armor/dearmor |
| 3 | 系统 gpg 集成 | 独立 `openpgp.gpg.*` 命令族：detect + exec + 高层包装（encrypt/decrypt/sign/verify/key.list/import/export） |
| 4 | gpg 命令族状态 | 完全独立于 sequoia 命令族，不读 credentials、不碰插件钥匙串、不依赖 passphrase 字段 |
| 5 | 密钥存储 | credentials.d 目录 + `type:"directory"` 自动注入；插件自管 `keys/<fp>.{pub,sec}.asc` + `index.json` |
| 6 | credentials 扩展 | 新增 `type:"file"`（dyyl 读文件内容注入）+ `type:"directory"`（dyyl 注入绝对路径） |
| 7 | Passphrase 来源 | credentials.toml `[plugin.openpgp].passphrase` 默认 + 命令调用时显式覆盖 |
| 8 | 默认密码套件 | 现代：Ed25519/Curve25519 + AES-256 + AEAD，输出 ASCII armor |
| 9 | 大数据处理 | 字符串命令 + `.file` 变体双模式（小数据走 JSON 内存，大文件插件直读直写） |
| 10 | verify 返回值 | dict `{valid, signer_uid, signer_fp}` 而非简单字符串 |
| 11 | 插件源码位置 | dyyl 主仓新建 `plugins/openpgp/` 目录 |
| 12 | ABI 版本 | 不升级，仍 ABI v2（15 符号），file/directory 是 manifest 扩展 |
| 13 | 指南形态 | 单文件 `docs/plugin-development-guide.md`，14 章，教程+参考一体 |
| 14 | 指南语言 | 中文，代码注释中文 |
| 15 | 信任模型 | 插件/脚本无限信任，不讨论防御性安全 |

---

## 3. 架构概览

### 3.1 插件运行时形态

- Cargo cdylib crate `plugins/openpgp/`，`crate-type = ["cdylib"]`，`panic = "abort"`
- 依赖：`sequoia-openpgp`、`serde`、`serde_json`、`anyhow`、`chrono`、`shell-words`、`which`
- 导出 15 个 ABI 符号（ABI v2，含 `set_credentials`），`handle_command` 内部分发到具体子命令
- 插件 handle 持有 `PluginState`：默认 passphrase、默认指纹、credentials 目录路径、已解锁私钥内存缓存、keyring 索引懒加载缓存

### 3.2 credentials 注入

dyyl 在 `on_load` 前调 `set_credentials(handle, json)`，JSON 形如：

```json
{
  "passphrase": "default-pass",
  "default_key": "ABCD1234EF567890",
  "__credentials_dir": "/home/user/.local/share/dyyl/credentials.d/openpgp"
}
```

- `passphrase` / `default_key`：来自 `credentials.toml` 的 `[plugin.openpgp]` 段（`type: "string"` 字段）
- `__credentials_dir`：来自 manifest `type: "directory"` 字段，dyyl 自动注入每插件（manifest 不显式声明也注入），指向 `~/.local/share/dyyl/credentials.d/openpgp/`

### 3.3 数据流（加密为例）

```
脚本: set $c, openpgp.encrypt $msg, $fp
  ↓
dispatch: plugin_name="openpgp", sub="encrypt", args=[msg, fp]
  ↓
PluginManager.dispatch → handle_command(handle, "encrypt", args_json, &out)
  ↓
插件内：
  1. 解析 args_json → [Str(msg), Str(fp_or_armored)]
  2. 若 fp 形如指纹 → 从 <credentials_dir>/keys/<fp>.pub.asc 读公钥
     否则当 armored 公钥解析
  3. sequoia-openpgp 构造 Message，AES-256 + AEAD，armor 输出
  4. 返回 {"type":"str","value":"-----BEGIN PGP MESSAGE-----\n..."}
  ↓
脚本: $c 现在是 armored 密文
```

### 3.4 gpg 命令族独立性约束

`openpgp.gpg.*` 命令族是系统 gpg 二进制的纯包装：
- 不读 `PluginState.default_passphrase` / `default_key` / `credentials_dir`
- 不读写 `credentials.d/openpgp/keys/`
- 不依赖 sequoia-openpgp crate 的任何功能
- 使用系统 `~/.gnupg` keyring（由 gpg 二进制自身管理）

这是设计约束：gpg.* 是逃生舱口，与 sequoia 命令族完全解耦，两者密钥不共享。

---

## 4. 完整命令清单

### 4.1 sequoia 实现的命令族 `openpgp.*`

| 命令 | arity | 签名 | 返回 | 失败哨兵 |
|---|---|---|---|---|
| `openpgp.key.generate` | 2 | `(user_id, passphrase?)` → 生成 Ed25519 主+Curve25519 加密子密钥 | 指纹字符串 | `""` |
| `openpgp.key.import` | 1 | `(armored)` → 入库（自动识别公/私） | 指纹字符串 | `""` |
| `openpgp.key.export` | 2 | `(fingerprint, secret?)` → secret=1 导出私钥 | armored 字符串 | `""` |
| `openpgp.key.list` | 0 | 列出库内所有密钥 | list of dict `[{fp, uid, secret, created}]` | `[]` |
| `openpgp.key.delete` | 1 | `(fingerprint)` → 删库内密钥（同时删 pub+sec） | `"1"` 成功 / `"0"` | `"0"` |
| `openpgp.encrypt` | ≥2 | `(text, recipient_fp_or_armor, ...)` → 多收件人 | armored 密文 | `""` |
| `openpgp.encrypt.file` | ≥3 | `(in_path, out_path, recipient, ...)` → 文件版 | `"1"`/`"0"` | `"0"` |
| `openpgp.decrypt` | 1+ | `(armor, passphrase?)` | 明文字符串 | `""` |
| `openpgp.decrypt.file` | 2+ | `(in_path, out_path, passphrase?)` | `"1"`/`"0"` | `"0"` |
| `openpgp.sign` | 2+ | `(text, key_fp, detach?, passphrase?)` → detach=1 分离签名 | armored 签名 | `""` |
| `openpgp.sign.file` | 3+ | `(in_path, out_path, key_fp, detach?, passphrase?)` | `"1"`/`"0"` | `"0"` |
| `openpgp.verify` | 1-2 | `(text_or_sig, signed_text?, key_armor?)` → 单参=内联验签（sig+data 合一），双参=分离验签（sig + 原文） | dict `{valid, signer_uid, signer_fp}` | `{"valid":"0"}` |
| `openpgp.verify.file` | 1-2 | `(sig_or_data_path, data_path?, key_armor?)` → 单参=内联验签文件，双参=分离验签（sig 文件 + 数据文件） | 同上 dict | `{"valid":"0"}` |
| `openpgp.sym.encrypt` | 2+ | `(text, passphrase, cipher?)` → 对称加密 | armored 密文 | `""` |
| `openpgp.sym.decrypt` | 2+ | `(armor, passphrase)` | 明文 | `""` |
| `openpgp.armor` | 1 | `(binary_b64)` → 二进制转 armor | armor 字符串 | `""` |
| `openpgp.dearmor` | 1 | `(armor)` → armor 转 base64 二进制 | b64 字符串 | `""` |

**多级命令路由**：所有命令在 manifest `commands[].name` 显式声明（含点号的多级名），`handle_command` 内部 `match cmd_name { "key.generate" => ..., "gpg.detect" => ..., ... }`。传给插件的 `cmd_name` 是去掉插件名前缀后的完整子命令路径（如 `key.generate`、`gpg.detect`），可含点号。

**Passphrase 解析优先级**（decrypt / sign 等命令）：
1. 命令显式参数传了 passphrase → 用调用值
2. 命令参数填 `_` 或省略 → 用 `PluginState.default_passphrase`（来自 credentials.toml）
3. 都没有 → `passphrase_wrong` 或 `runtime` 错误

### 4.2 系统 gpg 命令族 `openpgp.gpg.*`（独立、不读 credentials）

| 命令 | arity | 行为 | 返回 | 失败 |
|---|---|---|---|---|
| `openpgp.gpg.detect` | 0 | `gpg --version` 探测 | dict `{installed:"1"/"0", path, version}` | `{installed:"0"}` |
| `openpgp.gpg.exec` | 1+ | `(args_str)` 或 `(args_list)` 透传给 `gpg` 二进制 | stdout 字符串 | `""` |
| `openpgp.gpg.encrypt` | 2 | `(text, recipient)` → `gpg --armor --encrypt --recipient <r>` + stdin=text | armored 密文 | `""` |
| `openpgp.gpg.encrypt.file` | 3 | `(in_path, out_path, recipient)` → `gpg --encrypt --recipient <r> -o <out> <in>` | `"1"`/`"0"` | `"0"` |
| `openpgp.gpg.decrypt` | 1 | `(armor)` → `gpg --decrypt` + stdin | 明文 | `""` |
| `openpgp.gpg.decrypt.file` | 2 | `(in_path, out_path)` → `gpg --decrypt -o <out> <in>` | `"1"`/`"0"` | `"0"` |
| `openpgp.gpg.sign` | 2+ | `(text, key_id, detach?)` → `gpg --armor --local-user <k> (--detach-sign)? --sign` | armored 签名 | `""` |
| `openpgp.gpg.sign.file` | 3+ | `(in_path, out_path, key_id, detach?)` | `"1"`/`"0"` | `"0"` |
| `openpgp.gpg.verify` | 2 | `(sig_or_text, data?)` → 单参内联、双参分离 | dict `{valid, signer}` | `{valid:"0"}` |
| `openpgp.gpg.verify.file` | 2+ | `(sig_path, data_path?)` | 同上 dict | `{valid:"0"}` |
| `openpgp.gpg.key.list` | 0 | `gpg --list-keys --with-colons` 解析 | list of dict | `[]` |
| `openpgp.gpg.key.import` | 1 | `(armor)` → `gpg --import` + stdin | 导入计数字符串 `"3"` | `"0"` |
| `openpgp.gpg.key.export` | 2 | `(key_id, secret?)` → `gpg --armor (--export-secret-keys)? <k>` | armored | `""` |

**gpg.* 不提供 key.generate / key.delete**：key 生成让用户用 sequoia 的 `openpgp.key.generate`，或通过 `openpgp.gpg.exec "--gen-key ..."` 逃生舱口；key 删除走 `openpgp.gpg.exec "--delete-keys <k>"`。

**gpg.exec 参数处理**：
- args 是 Str 时用 `shell-words` crate 按 shell 风格 split
- args 是 List 时每个元素当一个参数
- 失败行为统一：gpg 退出码非 0 → 返回空字符串哨兵 + `--debug` 输出 gpg 的 stderr

---

## 5. credentials 系统扩展（file + directory 类型）

### 5.1 现状回顾

当前 `credentials.toml` `[plugin.<name>]` 段只支持 `String → String` 短字段。`set_credentials` 把整段以 JSON 对象注入插件。对 OpenPGP 这种需要大型/动态凭据的场景不够：armored 私钥动辄几 KB 含换行，塞进 TOML 单行字符串难编辑；密钥动态增删需要可写位置。

### 5.2 新增字段类型

`manifest` 的 `credentials.fields[].type` 扩展取值：

| type | manifest 声明 | dyyl 行为 | JSON 注入值 |
|---|---|---|---|
| `"string"`（默认） | 现有 | 读 `credentials.toml` 对应字段 | 字符串原值 |
| `"file"` | 新增 | 读 `~/.local/share/dyyl/credentials.d/<plugin>/<field>` 文件内容 | 文件内容字符串（UTF-8）；文件不存在则空字符串 + debug 警告 |
| `"directory"` | 新增 | 确保 `~/.local/share/dyyl/credentials.d/<plugin>/` 存在（不存在则 mkdir 0700） | 目录绝对路径字符串 |

**字段命名约定**：
- `type: "directory"` 的字段名约定为 `__credentials_dir`（双下划线前缀表"系统注入"）
- dyyl 自动为每个有 credentials 声明的插件注入此字段（即使 manifest 不显式声明也注入）
- `type: "file"` 字段名由插件自定义（如 `default_pubkey`、`revocation_cert`）

### 5.3 OpenPGP 插件的 credentials 声明

```jsonc
"credentials": {
  "fields": [
    {"name": "passphrase", "type": "string", "secret": true,
     "description": "Default passphrase for encrypted private keys in the plugin's keyring"},
    {"name": "default_key", "type": "string", "secret": false,
     "description": "Fingerprint of default key for sign/decrypt when not specified in command"},
    {"name": "__credentials_dir", "type": "directory", "secret": false,
     "description": "Plugin-scoped directory for large blobs (keys, etc.)"}
  ]
}
```

### 5.4 OpenPGP 插件的钥匙串布局

`__credentials_dir` 注入后路径形如 `/home/user/.local/share/dyyl/credentials.d/openpgp/`，插件在该目录下自管：

```
credentials.d/openpgp/
  keys/
    ABCD1234EF567890.pub.asc      # 公钥（按指纹命名）
    ABCD1234EF567890.sec.asc      # 私钥（passphrase 加密）
    FEDCBA0987654321.pub.asc
    FEDCBA0987654321.sec.asc
  index.json                       # 插件维护的索引：fp → uid/created/has_secret
```

- `key.generate` / `key.import` 写入 `keys/<fp>.{pub,sec}.asc` 并更新 `index.json`
- `key.list` 读 `index.json`（避免每次扫目录 + 解析 armored）
- `key.delete` 删文件 + 更新 index
- `key.export` 读对应文件
- 加密/验签时按指纹查 `keys/<fp>.pub.asc`
- 解密/签名时按指纹查 `keys/<fp>.sec.asc`，用 credentials.passphrase（或调用参数覆盖）解锁

### 5.5 权限约定

- `credentials.d/<plugin>/` 创建时权限 0700（仅属主读写执行），与系统惯例一致
- `credentials.d/<plugin>/keys/*.sec.asc` 写入时权限 0600
- `--debug` 时检查 `credentials.d/` 与 `.sec.asc` 文件权限，过松输出提示（与现有 `credentials.toml` 行为一致，不强制 chmod 修正）
- 已存在目录权限非 0700 → 不修正 + debug 提示

### 5.6 私钥内存管理

- 私钥内存中解密后用完即 drop（sequoia 的 `KeyPair` 在作用域结束自动 drop）
- 同进程内的私钥缓存：插件 handle 持有 `HashMap<Fingerprint, KeyPair>` 缓存已解锁私钥，避免同脚本内反复 passphrase 解锁
- 脚本结束 `on_unload` 清空缓存

### 5.7 ABI 兼容性

- `type: "file"` / `"directory"` 是 manifest 层面的扩展，**不需要 ABI 版本升级**（仍是 ABI v2，`set_credentials` 签名不变）
- dyyl 主仓的 `manifest.rs` 增加对新 type 的识别（当前 `CredentialField.type` 是 `String`，已能容纳新值，只需在 `loader.rs` 注入逻辑里分 type 处理）
- 老 type `"string"` 行为完全不变，现有插件零影响

---

## 6. OpenPGP 插件内部架构

### 6.1 crate 结构

```
plugins/openpgp/
  Cargo.toml              # crate-type = ["cdylib"], panic = "abort"
  plugin.toml.in          # manifest 模板（发布脚本填 sha256/url）
  src/
    lib.rs                # 15 个 ABI 符号导出 + handle_command 总分发
    state.rs              # PluginState：handle 持有的运行时状态
    creds.rs              # credentials 解析 + 钥匙串路径管理
    keyring.rs            # 钥匙串 CRUD（读写 keys/*.asc + index.json）
    commands/
      mod.rs              # 子命令分发 match
      key.rs              # key.generate / import / export / list / delete
      encrypt.rs          # encrypt / encrypt.file / sym.encrypt
      decrypt.rs          # decrypt / decrypt.file / sym.decrypt
      sign.rs             # sign / sign.file
      verify.rs           # verify / verify.file
      armor.rs            # armor / dearmor
      gpg.rs              # gpg.* 全族（detect / exec / 高层包装）
    codec.rs              # dyyl Value JSON 编解码（与 dyyl value_codec 对称）
    error.rs              # 内部错误枚举 + 转换为 ABI 返回的 error JSON
  tests/                  # 插件内单元测试（独立 cargo test）
    key_tests.rs
    encrypt_tests.rs
    decrypt_tests.rs
    sign_verify_tests.rs
    armor_tests.rs
    codec_tests.rs
    gpg_tests.rs
```

### 6.2 PluginState（handle 指向的结构）

```rust
pub struct PluginState {
    /// 默认 passphrase（来自 credentials.toml [plugin.openpgp].passphrase）
    pub default_passphrase: Option<String>,
    /// 默认指纹（来自 [plugin.openpgp].default_key）
    pub default_key: Option<String>,
    /// credentials.d/openpgp/ 绝对路径
    pub credentials_dir: PathBuf,
    /// 已解锁私钥内存缓存（同进程内复用，避免反复 passphrase 解锁）
    /// KeyPair 是 sequoia 的非 Send 类型，缓存用 Mutex 保护
    pub key_cache: Mutex<HashMap<Fingerprint, KeyPair>>,
    /// keyring 索引（懒加载，首次 key.list/encrypt 等用时读 index.json）
    pub index: Mutex<Option<KeyringIndex>>,
}
```

`dyyl_plugin_init` 分配 `Box::new(PluginState::default())`，把裸指针返给 dyyl；所有后续 ABI 函数收到 `handle: *mut c_void` 都转回 `&mut PluginState`。`dyyl_plugin_shutdown` 把指针 `Box::from_raw` 回收。

### 6.3 handle_command 总分发

```rust
#[no_mangle]
pub extern "C" fn dyyl_plugin_handle_command(
    handle: *mut c_void,
    cmd: *const c_char,
    args: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    let state = unsafe { &mut *(handle as *mut PluginState) };
    let cmd = unsafe { CStr::from_ptr(cmd) }.to_str().unwrap_or("");
    let args_str = unsafe { CStr::from_ptr(args) }.to_str().unwrap_or("[]");
    let args: Vec<DyylValue> = match codec::decode_args(args_str) {
        Ok(v) => v,
        Err(e) => return error_out(out, "type_error", &format!("arg decode: {e}")),
    };

    let result = commands::dispatch(state, cmd, &args);
    match result {
        Ok(v) => { codec::encode_out(out, &v); 0 }
        Err(e) => { error_out(out, e.code(), &e.message()); 1 }
    }
}
```

`commands::dispatch` 内部 `match cmd { "key.generate" => key::generate(state, args), ... }`，子命令函数签名统一 `fn(state: &mut PluginState, args: &[DyylValue]) -> Result<DyylValue, PluginError>`。

### 6.4 DyylValue codec

插件内部用自己的 `DyylValue` 枚举（避免直接依赖 dyyl runtime crate），与 dyyl 的 `Value` JSON 编码一一对应：

```rust
pub enum DyylValue {
    Num(String),    // 字符串形式，保留任意精度
    Str(String),
    Empty,
    List(Vec<DyylValue>),
    Dict(Vec<(DyylValue, DyylValue)>),  // 与 dyyl 一致：保序 KV 对
}
```

编解码与 [src/runtime/plugin/value_codec.rs](file:///workspace/src/runtime/plugin/value_codec.rs) 对称：
- `decode_args(json: &str) -> Result<Vec<DyylValue>>` 解析 dyyl 传来的 args JSON 数组
- `encode_out(out: *mut *mut c_char, v: &DyylValue)` 编码单个值为 JSON 并写出参
- `Expr` 类型在插件侧按 `Num` 处理（与 dyyl value_codec 一致：expr roundtrip best-effort 解析为 num）

### 6.5 错误模型

```rust
pub struct PluginError {
    code: &'static str,
    message: String,
}
```

`error_out(out, code, message)` 写 `{"code":"<code>","message":"<message>"}` 到 out，返回非 0。dyyl 收到非 0 后转 RuntimeError + 哨兵（与现有插件错误处理一致）。

约定 code 枚举（在 ABI 错误对象基础上扩展 OpenPGP 专属码）：

| code | 含义 |
|---|---|
| `arity_mismatch` | 参数数量不符 |
| `type_error` | 参数类型不对 |
| `unknown_command` | 未知子命令 |
| `runtime` | 插件内部错误 |
| `key_not_found` | 指纹在钥匙串中不存在 |
| `passphrase_wrong` | passphrase 无法解锁私钥 |
| `parse_failed` | armored 输入解析失败 |
| `verify_failed` | 验签失败（签名无效/未找到签名者） |
| `gpg_not_installed` | `openpgp.gpg.*` 调用但系统未装 gpg |
| `gpg_exec_failed` | gpg 二进制执行失败（退出码非 0） |

### 6.6 gpg 命令族实现要点

- `gpg.detect`：`std::process::Command::new("gpg").arg("--version")`，捕获 stdout；找到则正则取第一行版本号，path 字段用 `which` crate 拿绝对路径。任何失败（命令不存在、退出码非 0）→ 返回 `{"installed":"0"}`，不报错
- `gpg.exec`：args 是 Str 时用 `shell-words` crate split；是 List 时每个元素当一个参数。捕获 stdout/stderr/退出码，失败返回空字符串 + `--debug` 输出 stderr
- 高层包装（`gpg.encrypt` 等）：内部组装 gpg 参数 + 调 `Command`，复用 `gpg.exec` 的执行核心
- **所有 gpg.* 命令都不读 `PluginState` 的 credentials/keyring**，纯系统调用。这是设计约束（见 §3.4）

---

## 7. 构建与发布

### 7.1 Cargo.toml

```toml
[package]
name = "openpgp-plugin"
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-or-later"

[lib]
name = "openpgp"          # 产物 libopenpgp.so / openpgp.dll / libopenpgp.dylib
crate-type = ["cdylib"]

[dependencies]
sequoia-openpgp   = { version = "2", default-features = false, features = ["compression-deflate"] }
serde             = { version = "1", features = ["derive"] }
serde_json        = "1"
anyhow            = "1"
chrono            = { version = "0.4", default-features = false, features = ["clock"] }
shell-words       = "1"     # gpg.exec 参数 split
which             = "6"     # gpg.detect 找 gpg 路径

[profile.release]
panic = "abort"            # 跨 FFI 边界 panic 必须 abort（spec §7.2 UB 警告）
opt-level = 3
lto = true
```

**features 说明**：`default-features = false` + 只开 `compression-deflate` 让 dyyl 在编译期选默认 backend（通常是 rustls + nettle-less 纯 Rust）。实现时验证：若关掉 default-features 后缺 crypto backend，则调整为 `default-features = true`。

### 7.2 plugin.toml.in（manifest 模板）

```toml
# plugins/openpgp/plugin.toml.in —— 发布脚本填入 version/sha256/url 后生成 manifest.json
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

# ... 其余 27 条命令同 §4.1/§4.2 表 ...

[[commands]]
name = "gpg.detect"
arity = 0
brief = "Detect system gpg installation, return {installed, path, version}"

[[credentials.fields]]
name = "passphrase"
type = "string"
secret = true
description = "Default passphrase for encrypted private keys"

[[credentials.fields]]
name = "default_key"
type = "string"
secret = false
description = "Fingerprint of default key"

[[credentials.fields]]
name = "__credentials_dir"
type = "directory"
secret = false
description = "Plugin-scoped directory for large blobs"
```

### 7.3 构建

```bash
# 单平台构建（开发）
cd plugins/openpgp
cargo build --release
# 产物：target/release/libopenpgp.so

# 拷到 dist 供 server.js 分发
mkdir -p ../../dist/plugins/openpgp/0.1.0/linux-x86_64
cp target/release/libopenpgp.so ../../dist/plugins/openpgp/0.1.0/linux-x86_64/
```

### 7.4 发布脚本（扩展 scripts/publish-plugin.sh）

现有 `scripts/publish-plugin.sh` 需扩展支持：
1. 接收插件源码目录参数：`./scripts/publish-plugin.sh plugins/openpgp`
2. 读 `plugin.toml.in` 拿 name/version/commands/credentials
3. 拷贝 `target/release/lib<name>.so` 到 `dist/plugins/<name>/<version>/<platform>/`
4. 计算每个产物的 SHA256
5. 生成 `dist/plugins/<name>/manifest.json`，url 字段：
   - 开发环境：`http://localhost:8951/plugins/<name>/<version>/<platform>/lib<name>.so`
   - 生产环境：`https://l.dyyapp.com/plugins/<name>/<version>/<platform>/lib<name>.so`（脚本根据环境变量 `DYRL_DIST_HOST` 切换）
6. 跨平台发布：脚本支持 `--target <rustc-target>` 多次调用，每次 `cargo build --release --target <t>` 后拷产物 + 算 sha256 + 合并到 manifest.platforms 数组

### 7.5 server.js 路由

现有 [server.js](file:///workspace/server.js) 的 `/plugins/<name>/manifest.json` 与 `/plugins/<name>/<version>/<platform>/<filename>` 路由（plugin-ecosystem spec §8.2）已支持，无需改动 —— 把构建产物按 §7.3 拷到 `dist/plugins/openpgp/0.1.0/linux-x86_64/libopenpgp.so`，server.js 即可分发。

### 7.6 CI 集成

主仓 CI 加一个 job：
1. `cargo build --release`（主 dyyl）
2. `cd plugins/openpgp && cargo build --release && cargo test`（OpenPGP 插件单元测试）
3. 拷产物到 dist + 运行 `scripts/publish-plugin.sh plugins/openpgp` 生成 manifest
4. 跑 dyyl 主仓的 `tests/plugin_e2e_tests.rs`（需扩展：加 `openpgp` 插件的 e2e 用例，见 §8）

### 7.7 版本与 ABI

- 插件首个版本 `0.1.0`，`abi_version = 2`（与 dyyl 当前 ABI 一致）
- `dyyl_min = "0.2.0"`（dyyl 当前版本）
- sequoia-openpgp 主版本升级时（如 2 → 3）插件 minor 版本 +1，不破坏 ABI（sequoia API 变化是插件内部事，对 dyyl 透明）

---

## 8. 测试策略

### 8.1 测试分层

| 层 | 位置 | 目的 | 工具 |
|---|---|---|---|
| 单元（插件内部） | `plugins/openpgp/tests/` | 各命令独立逻辑、keyring CRUD、codec、错误码 | `cargo test`（插件 crate 独立） |
| 单元（dyyl credentials 扩展） | `tests/credentials_tests.rs` 扩展 | file/directory 类型注入、权限、缺失文件处理 | `cargo test` + tempdir |
| 集成（dyyl ↔ 插件） | `tests/openpgp_plugin_tests.rs` 新增 | 真实 dlopen + handle_command 全命令族 | `cargo test` + 已构建的 `libopenpgp.so` |
| e2e（脚本端到端） | `tests/fixtures/openpgp-*.dyyl` + golden | 脚本里调 `openpgp.*` 完整流程 | `cargo test` + golden fixture |
| gpg 集成 | `tests/openpgp_gpg_tests.rs` | gpg.detect + 高层包装（CI 容器装 gpg） | `cargo test` + 系统 gpg |

### 8.2 测试用例清单

**插件内部单元（`plugins/openpgp/tests/`）**

- `key_tests.rs`
  - `key.generate` 默认参数 → 返回 40 字符指纹，钥匙串目录有 `<fp>.sec.asc` + `<fp>.pub.asc` + index.json 更新
  - `key.generate` 自定义 passphrase → 私钥用该 passphrase 加密
  - `key.import` 公钥 armored → 入库为 `.pub.asc`，index 标 `has_secret=false`
  - `key.import` 私钥 armored → 入库为 `.sec.asc`，index 标 `has_secret=true`
  - `key.import` 重复指纹 → 覆盖（实现时定具体策略）
  - `key.import` 无效 armored → `parse_failed` 错误码
  - `key.export` 公钥 → 返回 armored 公钥
  - `key.export` secret=1 → 返回 armored 私钥（passphrase 加密）
  - `key.list` → list of dict，字段齐全
  - `key.delete` 存在的指纹 → 删文件 + 更新 index
  - `key.delete` 不存在的指纹 → `key_not_found` 错误码

- `encrypt_tests.rs`
  - `encrypt(text, fp)` → armored 密文，能用 `decrypt` 解回原文
  - `encrypt(text, armored_pubkey)` → 内联公钥也能加密
  - `encrypt` 多收件人 → 任一收件人私钥都能解
  - `encrypt` 不存在指纹 → `key_not_found`
  - `encrypt.file` → 文件写入 armored 密文
  - `sym.encrypt(text, pass)` → 对称加密，`sym.decrypt` 解回
  - `sym.encrypt` 指定老 cipher（CAST5）→ 仍能解

- `decrypt_tests.rs`
  - `decrypt(armor)` 用 credentials 默认 passphrase → 解回原文
  - `decrypt(armor, override_pass)` → 用覆盖 passphrase 解
  - `decrypt` 错误 passphrase → `passphrase_wrong`
  - `decrypt` 非密文 → `parse_failed`
  - `decrypt.file` → 文件解密

- `sign_verify_tests.rs`
  - `sign(text, fp)` 内联签名 → `verify` 返回 `{valid:"1", signer_fp:...}`
  - `sign(text, fp, detach=1)` 分离签名 → `verify(sig, text)` 验证
  - `sign` 错误 passphrase → `passphrase_wrong`
  - `verify` 篡改文本 → `{valid:"0"}` + `verify_failed` 错误码
  - `verify` 未找到签名者公钥 → `key_not_found`
  - `sign.file` / `verify.file` → 文件版

- `armor_tests.rs`
  - `armor(b64)` → PGP armor 头尾
  - `dearmor(armor)` → b64
  - `armor` + `dearmor` 往返

- `codec_tests.rs`
  - DyylValue 全类型编解码往返（Num/Str/Empty/List/Dict 嵌套）
  - 与 dyyl `value_codec.rs` 输出对称（用 dyyl 主仓的 `plugin_value_codec_tests.rs` fixture 对照）

- `gpg_tests.rs`（CI 容器需装 gnupg）
  - `gpg.detect` 装了 gpg → `{installed:"1", path, version}` 非空
  - `gpg.detect` PATH 上无 gpg（临时清空 PATH）→ `{installed:"0"}`
  - `gpg.exec "--version"` → 返回 gpg 版本字符串
  - `gpg.encrypt` + `gpg.decrypt` 往返 → 解回原文
  - `gpg.sign` + `gpg.verify` 往返
  - `gpg.key.list` → list of dict 非空（前提 CI 装了 gpg 且有默认 keyring；若无 key 则空 list，不报错）
  - `gpg.exec` gpg 退出码非 0 → 返回空字符串 + --debug stderr
  - `gpg.encrypt` 但系统无 gpg → `gpg_not_installed` 错误码

**dyyl credentials 扩展单元（`tests/credentials_tests.rs` 扩展）**

- manifest `type:"file"` 字段，文件存在 → 注入内容字符串
- manifest `type:"file"` 字段，文件不存在 → 注入空字符串 + debug 警告
- manifest `type:"directory"` 字段 → 注入绝对路径，目录自动创建（0700）
- `__credentials_dir` 即使 manifest 不显式声明也自动注入
- 已存在目录权限非 0700 → 不修正 + debug 警告
- `type:"string"` 老行为不变（回归测试）
- credentials.toml `[plugin.openpgp]` 段缺 `passphrase` → 触发交互提示补齐（mock stdin）

**集成（`tests/openpgp_plugin_tests.rs`，真实 dlopen）**

- 构建脚本 `tests/fixtures/build-openpgp.sh` 在测试前 `cd plugins/openpgp && cargo build --release`，产物拷到 tmpdir
- dlopen + 15 符号全解析
- `init(2)` → handle 非空
- `set_credentials(handle, json)` → 返回 0
- `on_load(handle)` → 返回 0
- 各命令通过 `handle_command` 调用 → 正确返回 Value
- 多级命令路由：`openpgp.key.generate` 的 cmd_name 是 `key.generate`（不含插件名前缀）
- `handle_command` 返回非 0 → dyyl 转 RuntimeError + 哨兵，脚本继续

**e2e（`tests/fixtures/openpgp-*.dyyl` golden）**

- `openpgp-roundtrip.dyyl`：generate → encrypt → decrypt → 验证原文一致
- `openpgp-sign-verify.dyyl`：generate → sign → verify → 验证 valid:"1"
- `openpgp-sym.dyyl`：sym.encrypt → sym.decrypt 往返
- `openpgp-gpg-detect.dyyl`：调 `gpg.detect`，输出 installed 字段
- `openpgp-keyring-persist.dyyl`：第一次脚本 generate 入库，第二次脚本 list 能看到（跨脚本持久化）
- `openpgp-fail-passphrase.dyyl`：错误 passphrase → 哨兵 + --debug stderr

### 8.3 测试基础设施

- **OpenPGP 插件 fixture 构建**：测试 `build.rs` 或 `tests/fixtures/build-openpgp.sh` 在测试前构建插件 cdylib。参考现有 `tests/fixtures/example-plugin/` 模式
- **隔离的 credentials 目录**：每个测试用 `tempdir` 设 `HOME` 环境变量，避免污染真实 `~/.local/share/dyyl/credentials.d/openpgp/`
- **gpg 测试隔离**：gpg.* 测试用 `tempdir` 设 `GNUPGHOME` 环境变量，避免污染真实 `~/.gnupg`
- **CI 矩阵**：Linux 容器装 `gnupg` 包；macOS/Windows CI 仅跑 sequoia 命令族，gpg.* 用 `#[cfg(target_os = "linux")]` 或 `ignore` 属性 gated

### 8.4 不测的

- 不测真实网络（sequoia 不联网，gpg.* 也不联网）
- 不测 sequoia 与 GnuPG 二进制的互操作（实现时如需可加，但 v1 不强制）
- 不测 Windows/macOS 实际 dlopen（CI 仅 Linux，其它靠平台条目选择逻辑单测覆盖）
- 不测 1800s 之外的极端性能（加密 GB 级文件）
- 不测 panic 跨 FFI UB（仅文档警告，与 plugin-ecosystem spec §7.2 一致）

### 8.5 与现有测试套件集成

- `cargo test` 一键跑全部（dyyl 主仓 + 插件 crate 各自 `cargo test`）
- `cargo fmt --check` + `cargo clippy --all-targets --all-features` 必须通过（项目 lint 严格：`unwrap_used`/`panic`/`indexing_slicing` 全 deny，插件代码需遵守）
- 插件 crate 的 `Cargo.toml` 加 `[lints.clippy]` 与主仓一致（或主仓 workspace lint 统一管理）

---

## 9. 插件开发指南

### 9.1 文件位置与定位

- 路径：`docs/plugin-development-guide.md`
- 单文件综合指南，既是教程（从头建 OpenPGP 插件）又是参考手册（ABI 全表、Value codec、manifest schema、credentials 全类型）
- 受众：会 Rust 的开发者，不假设熟悉 dyyl 内部
- 与现有 `dyyl-api-reference.md`（脚本作者向）区分：本指南是插件作者向

### 9.2 章节结构

```
# dyyl 插件开发指南

## 1. 简介
   - 什么是 dyyl 插件
   - 何时该写插件（vs 用 dyyl 脚本）
   - 插件能做什么/不能做什么（UB 风险预告）

## 2. 快速开始（30 行最小插件）
   - 创建 cdylib crate
   - 实现最小 15 符号
   - 构建为 .so
   - 用 server.js 本地分发测试
   - 在 dyyl 脚本里调用 `myplugin.hello`
   - 完整可运行代码（参考 tests/fixtures/example-plugin/）

## 3. 架构与生命周期
   - 插件调用数据流（dispatch → PluginManager → dlopen → handle_command）
   - 加载/卸载时机（首次调用 dlopen，脚本结束 on_unload + shutdown）
   - handle 的所有权（插件 init 返指针，shutdown 回收）
   - panic = "abort" 为什么必须

## 4. C ABI 契约（参考手册）
   - 15 符号全表（签名 + 作用 + 内存约定）
   - 符号导出模板代码（#[no_mangle] extern "C"）
   - 字符串内存约定（plugin malloc / dyyl 调 free_string）
   - ABI 版本兼容（v1 vs v2，set_credentials）

## 5. Value JSON 编解码
   - 6 种 Value 类型（Num/Str/Expr/Empty/List/Dict）
   - dyyl → 插件 args_json 数组格式
   - 插件 → dyyl out_json 单值格式
   - num 用字符串的原因（任意精度）
   - Rust 端 DyylValue 枚举示例代码
   - 嵌套结构示例

## 6. Manifest 与 plugin.toml
   - 远程 manifest.json schema 全字段
   - 本地 plugin.toml 副本
   - commands[].name 含点号的多级命令约定
   - platforms 多平台条目
   - abi_version / dyyl_min / panic_mode
   - has_locales 字段

## 7. Credentials 系统
   - 7.1 credentials.toml 结构（[ai] + [plugin.<name>]）
   - 7.2 manifest credentials.fields 声明
   - 7.3 三种字段类型：string / file / directory
   - 7.4 __credentials_dir 自动注入机制
   - 7.5 set_credentials ABI 调用时机（on_load 前）
   - 7.6 交互式提示流程（用户缺字段时）
   - 7.7 权限约定（0700 目录、0600 私钥、--debug 提示）
   - 7.8 大型/动态凭据模式（用 OpenPGP 钥匙串作范例）

## 8. i18n（插件双语）
   - locales/en.json + locales/zh.json 结构
   - manifest has_locales 字段
   - register_plugin 注册流程
   - 键命名约定（<plugin_name>.<key>）
   - zh 缺失 fallback 到 en
   - 插件返回的 message/brief/help 不被 dyyl 翻译

## 9. 构建、发布与分发
   - 9.1 Cargo.toml 配置（cdylib + panic=abort）
   - 9.2 单平台构建
   - 9.3 跨平台构建（cargo build --target）
   - 9.4 scripts/publish-plugin.sh 用法
   - 9.5 server.js 本地分发
   - 9.6 SHA256 校验流程
   - 9.7 版本号与 ABI 版本策略

## 10. 测试插件
   - 10.1 插件 crate 独立单元测试
   - 10.2 dyyl 主仓集成测试（dlopen fixture）
   - 10.3 e2e golden 脚本
   - 10.4 CI 集成
   - 10.5 lint 合规（clippy deny 规则）

## 11. 完整范例：OpenPGP 插件
   - 11.1 设计目标与命令清单
   - 11.2 crate 结构（plugins/openpgp/ 目录树）
   - 11.3 PluginState 设计（handle 持有状态 + Mutex）
   - 11.4 handle_command 分发
   - 11.5 key.generate 实现逐段讲解（sequoia-openpgp 用法）
   - 11.6 encrypt/decrypt 实现
   - 11.7 sign/verify 实现
   - 11.8 gpg.* 命令族实现（系统 gpg 集成）
   - 11.9 credentials.d 钥匙串管理
   - 11.10 错误码实践
   - 11.11 测试套件组织

## 12. 已知风险与约束
   - 12.1 信任模型：插件与脚本无限信任，与 dyyl 同权限（无沙箱）
   - 12.2 panic = "abort" 是硬性约束（跨 FFI panic 是 UB，非信任问题）
   - 12.3 credentials.toml 明文存储（事实，非威胁）
   - 12.4 私钥内存管理 best practice（drop 后不可访问，但非安全边界）
   - 12.5 gpg.* 命令族是系统调用包装，行为由 gpg 二进制决定

## 13. 故障排查
   - 常见错误码与含义
   - dlopen 失败（缺符号 / ABI 不匹配 / 平台条目缺）
   - SHA256 不符
   - credentials 提示循环
   - --debug 输出解读
   - 如何报告插件 bug

## 14. 参考资源
   - dyyl 主仓 README
   - dyyl-api-reference.md
   - 插件生态设计 spec
   - sequoia-openpgp 文档（OpenPGP 插件范例用）
   - RFC 4880 / 6637 / 9580（OpenPGP 标准）
```

### 9.3 写作原则

- **教程性章节（2、3、11）**：完整可运行代码，逐段讲解
- **参考性章节（4、5、6、7、8）**：表格 + schema + 简短示例，便于查阅
- **OpenPGP 插件作为贯穿范例**：第 11 章是完整真实代码讲解，前面的章节用最小示例；当某概念在 OpenPGP 插件里有更复杂应用时，前文用 "详见 §11.x" 交叉引用
- **代码块都加语言标签**（rust / toml / jsonc / bash / dyyl）
- **不复制 dyyl 主仓 spec 的内容**：用 "详见 [spec](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)" 链接交叉引用，避免内容漂移
- **i18n**：指南本身用中文写，代码注释也用中文

### 9.4 与现有文档的关系

- `README.md`：面向 dyyl 用户（脚本作者），不重复插件开发内容
- `dyyl-api-reference.md`：脚本 API 参考，新增 `openpgp.*` 命令族条目时引用本指南的"完整范例"章节
- `docs/superpowers/specs/`：设计 spec（本文件 + 现有三个），是设计决策记录；本指南是面向开发者的实操文档
- `docs/superpowers/plans/`：实现计划，writing-plans 产出后归档于此

### 9.5 维护策略

- 指南随插件代码同步更新：任何 ABI 变更、credentials 类型扩展、新命令族都要在指南对应章节反映
- OpenPGP 插件代码与第 11 章讲解保持同步：插件代码变更时检查第 11 章是否需更新

---

## 10. 实现顺序（高层，详细计划由 writing-plans 产出）

**阶段 1：credentials 系统扩展**

1. dyyl 主仓 `src/runtime/plugin/manifest.rs`：识别 `type: "file"` / `"directory"` 字段（当前 type 是 String，已能容纳，无需改结构）
2. dyyl 主仓 `src/runtime/plugin/loader.rs`：注入逻辑分 type 处理
   - `"string"` → 现有行为不变
   - `"file"` → 读 `credentials.d/<plugin>/<field>` 文件内容注入
   - `"directory"` → 确保 `credentials.d/<plugin>/` 存在（mkdir 0700），注入绝对路径
   - `__credentials_dir` 自动注入每插件（即使 manifest 不显式声明）
3. dyyl 主仓 `tests/credentials_tests.rs` 扩展：file/directory 类型用例
4. dyyl 主仓 `locales/en.json` + `locales/zh.json`：新增 credentials 类型相关 i18n 键（如 `plugin.credentials_dir_created`、`plugin.credentials_file_missing`）

**阶段 2：OpenPGP 插件骨架**

5. 新建 `plugins/openpgp/Cargo.toml` + `plugin.toml.in`
6. `plugins/openpgp/src/lib.rs`：15 个 ABI 符号导出（参考 `tests/fixtures/example-plugin/src/lib.rs`）
7. `plugins/openpgp/src/state.rs`：PluginState 结构 + init/shutdown 持有
8. `plugins/openpgp/src/codec.rs`：DyylValue 枚举 + 编解码
9. `plugins/openpgp/src/error.rs`：PluginError + error_out
10. `plugins/openpgp/src/creds.rs`：credentials JSON 解析 + 钥匙串路径
11. `plugins/openpgp/src/commands/mod.rs`：dispatch match 骨架

**阶段 3：sequoia 命令族实现**

12. `plugins/openpgp/src/keyring.rs`：钥匙串 CRUD + index.json
13. `plugins/openpgp/src/commands/key.rs`：key.generate / import / export / list / delete
14. `plugins/openpgp/src/commands/encrypt.rs`：encrypt / encrypt.file / sym.encrypt
15. `plugins/openpgp/src/commands/decrypt.rs`：decrypt / decrypt.file / sym.decrypt
16. `plugins/openpgp/src/commands/sign.rs`：sign / sign.file
17. `plugins/openpgp/src/commands/verify.rs`：verify / verify.file
18. `plugins/openpgp/src/commands/armor.rs`：armor / dearmor

**阶段 4：gpg 命令族实现**

19. `plugins/openpgp/src/commands/gpg.rs`：detect + exec + 高层包装（encrypt/decrypt/sign/verify/key.list/import/export）

**阶段 5：测试**

20. `plugins/openpgp/tests/` 全套单元测试（key/encrypt/decrypt/sign_verify/armor/codec/gpg）
21. dyyl 主仓 `tests/openpgp_plugin_tests.rs`：真实 dlopen 集成
22. dyyl 主仓 `tests/fixtures/openpgp-*.dyyl`：e2e golden 脚本
23. dyyl 主仓 `tests/openpgp_gpg_tests.rs`：gpg 集成测试
24. CI 配置：装 gnupg 包、构建插件、跑全套测试

**阶段 6：发布脚本与文档**

25. 扩展 `scripts/publish-plugin.sh` 支持 `plugins/<name>` 源码目录参数
26. 编写 `docs/plugin-development-guide.md`（14 章）
27. 更新 `README.md` 与 `dyyl-api-reference.md`：增 `openpgp.*` 命令族条目，引用指南

---

## 11. 开放问题（实现时再决）

- `key.import` 重复指纹的策略：覆盖 vs 拒绝 vs 版本化？v1 倾向覆盖（与 GnuPG `--import` 行为一致），实现时定。
- `verify` 多签名者情况：返回第一个有效签名者的信息，还是返回所有签名者列表？v1 返回第一个有效（dict），实现时若需求出现可改为 list of dict。
- `gpg.exec` 是否需要 stdin 数据支持（除了参数外传二进制数据给 gpg）？v1 仅参数透传，stdin 默认空；若需加密 stdin 数据，用 `gpg.encrypt` 等高层包装。实现时如需求出现可扩展 `gpg.exec` 支持 `__stdin` 字段。
- sequoia `default-features = false` 后是否真的能用纯 Rust backend？实现时验证，必要时调整为 `default-features = true`（接受 sequoia 默认 backend，可能引入 nettle 系统依赖）。
- 插件 crate 是否纳入 dyyl 主仓 Cargo workspace？v1 倾向独立 crate（不在主 workspace），避免 sequoia 依赖拖慢主仓编译；实现时若 workspace 管理更方便可调整。
- `index.json` 的并发安全：同进程内用 Mutex 保护，但跨进程（多个 dyyl 实例同时操作同一钥匙串）无锁。v1 文档警告，不实现文件锁。
- `gpg.detect` 的 Windows 路径：`which` crate 在 Windows 上找 `gpg.exe`，实现时验证。
- 指南第 11 章代码讲解的颗粒度：逐行 vs 逐段？v1 倾向逐段（关键逻辑块讲解），实现时根据代码量调整。
