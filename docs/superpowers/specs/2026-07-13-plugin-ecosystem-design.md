# dyyl 插件生态系统设计

**日期：** 2026-07-13
**状态：** 设计已批准，待写实现计划
**关联：** dyyl v0.2.0；为后续"内置命令迁移到插件"铺路

---

## 1. 目标与非目标

### 1.1 目标

- 在 dyyl 运行时中加入插件生态，目前仅支持官方源 `l.dyyapp.com`。
- 脚本里以 `<plugin_name>.<sub>[.<sub>...]+` 多级短名直接调用插件命令，无需 `use`/`import` 声明。支持任意深度（如 `migpt.greet`、`migpt.user.login`、`migpt.config.set.timeout`）。
- 首次调用时运行时自动从 `l.dyyapp.com` 下载并安装对应插件，然后继续运行脚本。
- 插件以动态库（.so/.dll/.dylib）形式分发，由 Rust 等编译型语言编写，通过 C ABI 与 dyyl 通信。
- 设计须支持"将来把内置命令迁移到插件"，迁移后调用体验与内置几乎无差别。
- dyyl 自带的开发服务器 `server.js` 须扩展支持插件分发路由，路径结构与生产 `l.dyyapp.com` 一致，便于本地开发与测试。
- 提供 CLI 子命令管理插件生命周期：`dyyl install <name>` / `dyyl update <name>` / `dyyl update` / `dyyl remove <name>` / `dyyl autoremove`。
- **前置工作**：重设计 i18n 基础设施——从"每条消息一个手写 `pub fn`"升级为"键值表 + JSON 资源文件 + fallback + 插件可注册"，作为插件系统的依赖项先行完成。当前 i18n.rs 的手写函数风格无法支撑插件双语、无法批量测试覆盖率、无法被插件复用。

### 1.2 非目标（v1）

- 不支持第三方源；不开放任意 URL 下载。
- 不做插件签名验证（PKI）；仅 HTTPS + SHA256 清单校验。
- 不做插件 panic 跨 FFI 隔离；仅文档警告 UB 风险。
- 不做版本锁定 / 锁文件；始终使用 latest。
- 不做权限/能力沙箱；插件拥有 dyyl 同等权限。
- 不实现脚本内 `plugin.install` 命令（管理操作只通过 CLI 子命令，不混入脚本语法）。

---

## 2. 关键决策汇总

| 维度 | 决策 |
|---|---|
| 插件物理形态 | 动态库 FFI（.so/.dll/.dylib） |
| 编写语言 | Rust 等编译型语言 |
| 调用语法 | `<name>.<sub>[.<sub>...]+` 多级，无声明，未知前缀自动当插件（如 `migpt.greet`、`migpt.user.login`） |
| 下载时机 | 首次调用惰性下载 |
| 版本策略 | 始终 latest |
| ABI 形态 | 富 ABI 多函数 + 生命周期钩子（13 函数） |
| Value 编码 | JSON 字符串 |
| manifest | 静态 plugin.toml（最小调度集） |
| 存储位置 | XDG data: `~/.local/share/dyyl/plugins/<name>/<version>/` |
| 信任机制 | HTTPS + SHA256 清单校验 |
| panic 隔离 | 不隔离，仅文档警告 |
| 生命周期 | 首次调用 dlopen，常驻到脚本结束 |
| 平台分发 | 清单含多平台条目 |
| 错误模型 | 返回码 + out JSON，与现有 RuntimeError 对齐 |
| CLI 管理子命令 | `install`/`update`/`remove`/`autoremove`，与 `--flag <filename>` 模式互斥 |
| 插件使用追踪 | config 记录每个插件的 `last_used_at`，autoremove 据此清理 30 天未用 |
| 双语适配 (i18n) | 自研键值表 + JSON 资源文件（locales/en.json + locales/zh.json）+ zh 缺失回退 en + 插件自带 JSON 注册到 dyyl |

---

## 3. 架构

### 3.1 新增模块

```
src/runtime/plugin/
  mod.rs          — PluginManager 入口，持有已加载插件表
  manifest.rs     — plugin.toml + 远程 manifest.json 解析
  registry.rs     — 已装插件注册表（扫描 XDG data 目录）
  abi.rs          — C ABI 类型与 extern "C" 签名
  loader.rs       — dlopen + 符号解析 + 调用
  fetch.rs        — 从 l.dyyapp.com 拉清单+产物+SHA256 校验（运行时与 CLI 共用）
  store.rs        — 存储路径管理（XDG data，运行时与 CLI 共用）
src/runtime/cmd/plugin.rs — 路由未知前缀到 PluginManager
src/cli/
  mod.rs          — CLI 子命令分发
  plugin_cmds.rs  — install/update/remove/autoremove/list 实现
locales/
  en.json         — 英文消息资源
  zh.json         — 中文消息资源
src/i18n.rs       — 重设计：MessageStore + t() + register_plugin()
```

### 3.2 改动点

- [src/runtime/cmd/dispatch.rs](file:///workspace/src/runtime/cmd/dispatch.rs) 末尾 `_` 分支前加 fallback：命令名含至少一个点号且首段非已知命令族时，按首个点号切分出 `plugin_name` 与 `sub`（可含点号），转交 `plugin::dispatch(plugin_name, sub, call, env, ctx)`。
- [src/runtime/env.rs](file:///workspace/src/runtime/env.rs) 的 `Env` 增 `plugin_manager: PluginManager` 字段。
- [src/config.rs](file:///workspace/src/config.rs) 增 `installed_plugins: HashMap<String, InstalledPluginRecord>` 跟踪已装版本、`installed_at`、`last_used_at`、`sha256`。
- [src/main.rs](file:///workspace/src/main.rs) 增子命令分发：首参为 `install`/`update`/`remove`/`autoremove`/`list` 时走 CLI 管理路径，否则保持现有 `--flag <filename>` 行为。
- 新增 `src/cli/plugin_cmds.rs`：CLI 子命令实现，复用 `runtime/plugin/fetch.rs` + `store.rs`。
- [src/i18n.rs](file:///workspace/src/i18n.rs) **重设计**为键值表 + JSON 资源加载（见 §12）。现有所有手写 `pub fn` 迁移为 `t(lang, key, args)` 调用，旧函数保留为薄包装避免一次性大改。
- 新增 `locales/en.json` + `locales/zh.json`：所有消息文案集中存放，键命名 `namespace.action`（如 `plugin.sha256_mismatch`、`cli.usage`）。

### 3.3 首次调用数据流

以脚本中出现 `migpt.user.login "u", "p"` 为例（migpt 尚未安装）：

```
1. dispatch 遇 "migpt.user.login"，按首个点号切分：plugin_name="migpt", sub="user.login"
   （sub 可含多个点号，整体作为子命令名传给插件）
2. 验证 plugin_name="migpt" 非已知命令族（math/str/logic/.../mcm/language/set/create.*）
3. fallback → plugin::dispatch("migpt", "user.login", call, env, ctx)
4. PluginManager 查 registry：migpt 已装？否
5. fetch.fetch_manifest("migpt")
     GET https://l.dyyapp.com/plugins/migpt/manifest.json
     → 含 {name, version, abi_version, dyyl_min, platforms, commands}
6. 校验 manifest：abi_version 兼容？dyyl_min 满足？当前平台有条目？
7. 下载产物到临时文件，计算 SHA256，与 manifest 平台条目 sha256 比对
8. 校验通过：原子重命名到
     ~/.local/share/dyyl/plugins/migpt/<version>/libmigpt.so
   并写 plugin.toml 副本（记录来源、版本、sha256）
9. registry 标记 migpt 已装；config.toml 更新 installed_plugins
10. loader.dlopen + 解析 14 个 ABI 符号（13 核心函数 + free_string）+ 调 plugin_init(DYYL_API_VERSION) -> handle
11. 调 list_commands(handle) 确认 "user.login" 存在 + arity 匹配
12. 调 on_load(handle) 生命周期钩子
13. 调 handle_command(handle, "user.login", args_json, &mut out_json) -> int
    （cmd_name 是去掉插件名前缀后的完整子命令路径，可含点号）
14. 解析 out JSON → Value，返回 dispatch
15. 脚本结束：调 on_unload(handle) + plugin_shutdown(handle) + dlclose
```

### 3.4 失败行为

任何步骤失败（网络、SHA256 不符、ABI 不兼容、符号缺失、`handle_command` 返回非 0）→ 按 dyyl 现有错误模型产出 `RuntimeError` + sentinel，脚本继续（与 `mcm.*` 失败行为一致）。

### 3.5 第二次调用

同一脚本内再次调用 `migpt.*`：

- registry 命中已装
- PluginManager 命中已加载 handle
- 直接 `handle_command`，无再 dlopen

### 3.6 已知前缀保护与多级命令路由

dispatch 路由逻辑（按顺序）：

1. 完全匹配（`language`、`set` 等）→ 内置
2. `create.num`、`create.str` → 内置
3. 按已知命令族前缀 `starts_with` 匹配：`math.`、`str.`、`logic.`、`dict.`、`list.`、`file.`、`net.`、`io.`、`user.`、`system.`、`time.`、`mcm.` → 内置
4. 命令名含至少一个点号，且**首段**（首个点号前）非上述已知族 → 当插件调用：
   - `plugin_name` = 首段
   - `sub` = 首个点号后的全部内容（可含多个点号）
   - 转交 `plugin::dispatch(plugin_name, sub, call, env, ctx)`
5. 命令名不含点号，或首段是已知族但完整命令名不被识别 → 现有 unknown command sentinel

**多级命令的子命令名约定：** 传给插件 `handle_command` 的 `cmd_name` 是**去掉插件名前缀后的完整子命令路径**。例如：

| 脚本里的命令 | plugin_name | cmd_name（传给插件） |
|---|---|---|
| `migpt.greet` | migpt | `greet` |
| `migpt.user.login` | migpt | `user.login` |
| `migpt.config.set.timeout` | migpt | `config.set.timeout` |

manifest 的 `commands[].name` 与 `list_commands` 输出的 name 也用此形式（可含点号），dyyl 据此做存在性校验与 arity 检查。

这保证"内置命令迁移到插件"是渐进式的：可按命令族逐个迁移，未迁移的仍走内置。例如将来 `math.*` 迁移到插件时，可让 `math` 不再是已知族，所有 `math.add` / `math.trig.sin` 都路由到 `math` 插件，子命令分别是 `add` / `trig.sin`——脚本调用语法不变。

---

## 4. C ABI 契约

### 4.1 ABI 符号清单（13 函数）

所有函数名以 `dyyl_plugin_` 前缀，C ABI（`extern "C"`），避免符号冲突。

| 函数 | 签名（简化） | 作用 |
|---|---|---|
| `dyyl_plugin_get_api_version` | `() -> u32` | 返回插件编译时针对的 dyyl 插件 API 版本（当前 = 1）。dyyl 启动时检查兼容。 |
| `dyyl_plugin_get_name` | `(*mut *mut c_char) -> int` | 通过出参返回插件名（与 manifest.name 必须一致）。插件 malloc 字符串，写出到参。 |
| `dyyl_plugin_get_version` | `(*mut *mut c_char) -> int` | 写插件版本字符串。 |
| `dyyl_plugin_get_author` | `(*mut *mut c_char) -> int` | 可选，空字符串也行。 |
| `dyyl_plugin_get_description` | `(*mut *mut c_char) -> int` | 可选。 |
| `dyyl_plugin_init` | `(api_version: u32) -> *mut c_void` | 初始化，返回插件 handle。NULL 表示失败。dyyl 调用前先验 `get_api_version` 兼容性。 |
| `dyyl_plugin_on_load` | `(handle: *mut c_void) -> int` | 加载完成钩子。返回 0=ok，非 0=失败码（此时 dyyl 拒绝该插件）。 |
| `dyyl_plugin_list_commands` | `(handle: *mut c_void, out: *mut *mut c_char) -> int` | 输出 JSON 数组 `[{"name":"greet","arity":1,"brief":"..."}, {"name":"user.login","arity":2,"brief":"..."}]`。`name` 可含点号表示多级子命令。dyyl 用于静态校验与 arity 检查。 |
| `dyyl_plugin_get_command_help` | `(handle: *mut c_void, cmd_name: *const c_char, out: *mut *mut c_char) -> int` | 单命令详细帮助字符串。`plugin.help migpt greet` 用。 |
| `dyyl_plugin_handle_command` | `(handle: *mut c_void, cmd_name: *const c_char, args_json: *const c_char, out_json: *mut *mut c_char) -> int` | 核心调度。`cmd_name` 是去掉插件名前缀后的完整子命令路径，**可含点号**（如 `user.login`、`config.set.timeout`）。args_json 是 dyyl Value 数组的 JSON。成功写 out_json（单个 Value 的 JSON），返回 0。失败写 out_json 为错误对象，返回非 0。 |
| `dyyl_plugin_on_error` | `(handle: *mut c_void, cmd_name: *const c_char, error_code: *const c_char, error_json: *const c_char) -> int` | dyyl 在调度失败后回调，插件可记录/清理。返回 0 忽略。 |
| `dyyl_plugin_on_unload` | `(handle: *mut c_void) -> int` | 卸载前钩子。 |
| `dyyl_plugin_shutdown` | `(handle: *mut c_void) -> ()` | 释放 handle。之后不再调用。 |

**内存约定：** 所有 `out: *mut *mut c_char` 是出参——插件用 `malloc` 分配字符串，把指针写到 `*out`。dyyl 用完后必须调 `dyyl_plugin_free_string(ptr)` 让插件自己 `free`。这样跨分配器安全（同一进程不同 Rust 版本的 allocator 可能不同）。dyyl 传给插件的 `*const c_char`（cmd_name、args_json 等）由 dyyl 拥有，插件只读，dyyl 在调用返回后保证有效。

第 14 个辅助函数 `dyyl_plugin_free_string(ptr: *mut c_char) -> ()` 是内存释放函数，与上述 13 个并列（实际共 14 个导出符号；前文"13 函数"指核心逻辑函数，不含 free_string）。

### 4.2 Value 的 JSON 编码

覆盖内置全类型，为"内置命令迁移到插件"保证无损往返：

```jsonc
// dyyl → 插件（args_json，始终是数组）
[ {"type":"num","value":"3"},                        // CasNumber 的字符串形式
  {"type":"str","value":"hi"},
  {"type":"expr","value":"1⅔"},                      // 符号表达式
  {"type":"empty"},
  {"type":"list","value":[...嵌套...]},
  {"type":"dict","value":{"k":...}} ]

// 插件 → dyyl（out_json，单个值）
{"type":"str","value":"hello from migpt"}
```

**为什么 `num` 用字符串而非 JSON number：** dyyl 的 `CasNumber` 支持任意大整数、分数、根式——JSON number 装不下。字符串保持精确，与 `mcm McmArg::Str` 序列化策略一致。

这与现有 `mcm` 协议的 `McmArg` 完全对称，迁移内置命令时 Value 来回无损。

### 4.3 错误对象

`handle_command` 失败时 out_json 写：

```jsonc
{ "code": "arity_mismatch",
  "message": "greet expects 1 arg, got 2",
  "line": 5,                // 可选，dyyl 填充
  "command": "migpt.greet" }
```

dyyl 把这个转成 `RuntimeError::new(line, command, message)` 并产出 sentinel，与内置命令失败完全一致。`code` 字段进 stderr 便于诊断。

**i18n 约定：** 插件返回的 `message` 是**插件自己的原文**，dyyl **不翻译**——插件作者负责自己消息的双语（可通过自己的 i18n 机制，或根据 dyyl 传来的 `lang` 提示选择）。dyyl 翻译的是自己生成的消息（manifest 校验失败、SHA256 不符、网络错误、ABI 不兼容、未知子命令前的路由提示等）。`code` 字段是机器可读枚举，不翻译。

**约定 code 枚举**（v1 最小集）：

| code | 含义 |
|---|---|
| `arity_mismatch` | 参数数量不符 |
| `type_error` | 参数类型不对 |
| `unknown_command` | 插件不认识这个子命令 |
| `runtime` | 插件内部运行错误 |
| `panic` | 插件 panic（不隔离时仅警告路径） |

---

## 5. Manifest 与存储

### 5.1 远程 manifest.json（l.dyyapp.com 提供）

URL 约定：`https://l.dyyapp.com/plugins/<name>/manifest.json`

```jsonc
{
  "name": "migpt",
  "version": "0.1.0",
  "abi_version": 1,
  "dyyl_min": "0.2.0",
  "panic_mode": "abort",
  "commands": [
    { "name": "greet",        "arity": 1, "brief": "Send a greeting" },
    { "name": "auth",         "arity": 2, "brief": "Login with user/password" },
    { "name": "user.login",   "arity": 2, "brief": "User login (multi-level)" },
    { "name": "user.logout",  "arity": 0, "brief": "User logout" },
    { "name": "config.set",   "arity": 2, "brief": "Set config key" }
  ],
  "platforms": [
    { "platform": "linux-x86_64",   "url": "https://l.dyyapp.com/.../libmigpt.so",       "sha256": "abc..." },
    { "platform": "linux-aarch64",  "url": "https://l.dyyapp.com/.../libmigpt_aarch64.so","sha256": "def..." },
    { "platform": "macos-aarch64",  "url": "https://l.dyyapp.com/.../libmigpt.dylib",    "sha256": "ghi..." },
    { "platform": "windows-x86_64", "url": "https://l.dyyapp.com/.../migpt.dll",         "sha256": "jkl..." }
  ]
}
```

### 5.2 本地 plugin.toml（安装后副本）

存储于 `~/.local/share/dyyl/plugins/<name>/<version>/plugin.toml`：

```toml
name = "migpt"
version = "0.1.0"
abi_version = 1
dyyl_min = "0.2.0"
panic_mode = "abort"

[[commands]]
name = "greet"
arity = 1
brief = "Send a greeting"

[[commands]]
name = "user.login"
arity = 2
brief = "User login (multi-level)"

[[commands]]
name = "config.set"
arity = 2
brief = "Set config key"

[installed]
source_url = "https://l.dyyapp.com/.../libmigpt.so"
sha256 = "abc..."
installed_at = "2026-07-13T10:30:00Z"
dyyl_version = "0.2.0"
```

### 5.3 存储目录结构

```
~/.local/share/dyyl/plugins/
  migpt/
    0.1.0/
      libmigpt.so         # 或 migpt.dll / libmigpt.dylib
      plugin.toml         # 安装时写入的本地副本
    0.2.0/                # 升级时并存（始终 latest 时新版本安装后旧版本可清理）
      ...
  other_plugin/
    ...
```

### 5.4 兼容性校验（安装前 gate）

下载前 dyyl 依次检查：

1. `abi_version` == 当前 dyyl 支持的 API 版本（v1 = 1）。不等 → 拒绝。
2. `dyyl_min` ≤ 当前 dyyl 版本。不满足 → 拒绝。
3. 当前 `(os, arch)` 在 `platforms` 中有条目。无 → 拒绝。

任一失败 → RuntimeError + sentinel，不下载。

### 5.5 已安装插件记录（config.toml 扩展）

`DyylConfig` 新增字段：

```toml
[installed_plugins.migpt]
version = "0.1.0"
installed_at = "2026-07-13T10:30:00Z"
sha256 = "abc..."
```

用于启动时快速判断哪些插件已装、避免每次脚本运行都重扫 XDG data 目录。

---

## 6. 生命周期与版本

### 6.1 加载/卸载时机

- 首次调用 `<name>.<sub>...` 时 `dlopen` 并 `plugin_init` → 常驻。
- 之后同脚本内再次调用复用 handle。
- 脚本结束时依次：`on_unload` → `plugin_shutdown` → `dlclose`。

### 6.2 "始终 latest" 策略

每次脚本运行中首次调用某插件命令时（无论该插件本地是否已装），dyyl 都从 l.dyyapp.com 拉一次 manifest：

- **本地未装** → 按 manifest 下载安装。
- **本地已装且 manifest.version == 本地版本** → 直接加载已装版本，不重复下载。
- **本地已装但 manifest.version ≠ 本地版本** → 下载新版本到新版本目录，更新 config 记录，加载新版本（旧版本目录保留，见 §10 开放问题）。

manifest 不做本地缓存——每次脚本运行都拉最新清单。代价：每次首次调用有网络延迟；好处：真正"始终 latest"。脚本行为可能因远程更新而漂移，用户已知并接受。不做锁文件。

### 6.3 ABI 版本兼容

- 当前 `DYYL_API_VERSION = 1`。
- 插件 `get_api_version()` 必须返回与 dyyl 当前一致才能加载。
- dyyl 升级 API 版本时允许破坏兼容，届时所有插件需重新编译。

---

## 7. 安全注意事项

### 7.1 信任边界

- 仅信任 `l.dyyapp.com`（HTTPS）。
- 下载产物必须 SHA256 匹配 manifest 平台条目声明的哈希。
- 不做代码签名；不做运行时沙箱。

### 7.2 UB 风险警告（必须文档化）

动态库跨 FFI 边界 panic 是 **未定义行为**。虽然 manifest 声明 `panic_mode = "abort"` 是强烈建议（插件以 `panic=abort` 编译，panic 时直接 abort 进程而非 unwind），但 dyyl 无法强制验证插件二进制是否真的 `panic=abort` 编译。

后果：

- 插件 panic 可能导致 dyyl 进程崩溃、内存损坏、或不可预测行为。
- 用户安装非官方来源或被篡改的插件自负风险（v1 仅官方源缓解了这点）。

文档与 `--debug` 输出须明示此风险。

### 7.3 平台条目选择

dyyl 按 `(std::env::consts::OS, std::env::consts::ARCH)` 构造平台标识：

- `linux-x86_64`、`linux-aarch64`
- `macos-x86_64`、`macos-aarch64`
- `windows-x86_64`、`windows-aarch64`

清单无当前平台条目 → 拒绝安装，错误信息列出清单支持的平台。

---

## 8. 开发服务器扩展（server.js）

### 8.1 背景

`server.js` 是 dyyl 仓库自带的 Node.js 开发/分发服务器（端口 8951），现仅分发 dyyl 二进制本体。为支持插件生态，须扩展为也分发插件，路径结构与生产 `l.dyyapp.com` 完全一致——这样开发时用 `localhost:8951`，生产用 `l.dyyapp.com`，dyyl 客户端代码无需区分。

### 8.2 新增路由

在现有 `/`、`/install`、`/download` 基础上新增：

| 方法 | 路径 | 响应 | 说明 |
|---|---|---|---|
| GET | `/plugins/<name>/manifest.json` | `application/json` | 返回该插件的远程 manifest.json（§5.1 schema） |
| GET | `/plugins/<name>/<version>/<platform>/<filename>` | `application/octet-stream` | 返回插件二进制产物 |

`<name>`、`<version>`、`<platform>`、`<filename>` 均为路径参数，服务器从对应磁盘目录读取并返回。任何不匹配上述模式的路径仍返回 403（保持现有"白名单"策略）。

### 8.3 磁盘目录结构（开发服务器侧）

```
dist/
  dyyl                          # 现有：dyyl 二进制本体
  plugins/
    migpt/
      manifest.json             # 该插件的远程 manifest
      0.1.0/
        linux-x86_64/
          libmigpt.so
        linux-aarch64/
          libmigpt.so
        macos-aarch64/
          libmigpt.dylib
        windows-x86_64/
          migpt.dll
      0.2.0/
        ...
    other_plugin/
      manifest.json
      ...
```

manifest.json 中的 `url` 字段在开发环境指向 `http://localhost:8951/plugins/<name>/<version>/<platform>/<filename>`；生产环境指向 `https://l.dyyapp.com/...`。manifest 由发布脚本生成（见 §8.5）。

### 8.4 SHA256 计算约定

发布脚本在打包时计算每个平台产物的 SHA256，写入 manifest.json 的 `platforms[].sha256`。服务器只是静态文件服务，不参与哈希计算——dyyl 客户端下载后自行校验。

### 8.5 发布脚本（新增）

新增 `scripts/publish-plugin.sh`（或 Node 等价物），输入一个已 `cargo build --release` 的插件 cdylib 目录，执行：

1. 读取插件 `Cargo.toml` 拿 name/version
2. 拷贝产物到 `dist/plugins/<name>/<version>/<platform>/`（按当前 host 平台；跨平台发布需多次构建）
3. 计算每个产物的 SHA256
4. 生成/更新 `dist/plugins/<name>/manifest.json`（合并 platforms 数组、commands 从插件导出的元数据读取或从源 `plugin.toml.in` 读取）

### 8.6 安全注意

开发服务器监听 `0.0.0.0`，生产部署时应：

- 仅监听内网或加反代
- 启用 HTTPS（生产 l.dyyapp.com 已是 HTTPS）
- manifest.json 与产物文件由可信构建流程写入，不接受运行时上传

### 8.7 测试

`tests/plugin_tests.rs` 的 fetch.rs 子用例用本地 HTTP server（Rust 内 tokio 启动）模拟 `l.dyyapp.com`，路径结构严格遵循 §8.2。这样 `server.js` 与 Rust 测试 server 走同一套路径契约，未来若 server.js 路径变化，两边同步更新。

---

## 9. CLI 子命令（插件管理）

### 9.1 子命令总览

`dyyl` CLI 新增"管理子命令"模式，与现有 `--flag <filename>` 脚本执行模式互斥。CLI 解析顺序：第一个位置参数若是已知子命令（`install`/`update`/`remove`/`autoremove`/`list`），走管理路径；否则按现有逻辑当 `<filename>`。

| 子命令 | 用途 | 用法 |
|---|---|---|
| `dyyl install <name>` | 显式下载并安装某插件（不运行脚本） | `dyyl install migpt` |
| `dyyl update <name>` | 升级某插件到 latest（拉 manifest，版本不同则下载新版本） | `dyyl update migpt` |
| `dyyl update` | 升级所有已装插件到 latest | `dyyl update` |
| `dyyl remove <name>` | 卸载某插件（删本地目录 + 清 config 记录） | `dyyl remove migpt` |
| `dyyl autoremove` | 卸载所有 `last_used_at` 超过 30 天的插件 | `dyyl autoremove` |
| `dyyl list` | 列出已装插件及版本 | `dyyl list` |

`list` 是辅助子命令，便于查看状态。`--lang` 仍可在子命令前使用以选择输出语言。

### 9.2 install 行为

1. 从 `https://l.dyyapp.com/plugins/<name>/manifest.json` 拉 manifest
2. 兼容性 gate（§5.4）：abi_version、dyyl_min、当前平台条目
3. 下载产物到临时文件 + SHA256 校验
4. 原子安装到 `~/.local/share/dyyl/plugins/<name>/<version>/`
5. 写本地 plugin.toml 副本 + 更新 config.toml 的 `installed_plugins.<name>` 记录（含 `installed_at` 与 `last_used_at`，见 §9.6）
6. 输出：`installed migpt 0.1.0` 或失败原因到 stderr，退出码 0/非 0

**幂等**：若已装且版本与 manifest 一致，输出 `migpt 0.1.0 already installed`，退出 0。若已装但版本不同，按 update 行为升级。

### 9.3 update 行为

**单插件 `dyyl update <name>`：**

1. config 命中已装？否 → `error: <name> not installed`，退出非 0
2. 拉 manifest，校验
3. manifest.version == 本地版本 → `<name> already latest (0.1.0)`，退出 0
4. 不同 → 下载新版本到新目录，更新 config 记录，**保留旧版本目录**（v1，见 §11 开放问题）
5. 输出：`updated migpt 0.1.0 -> 0.2.0`

**全部 `dyyl update`：**

1. 遍历 config.installed_plugins
2. 对每个插件执行单插件 update 逻辑
3. 汇总输出：每个插件一行结果，最后总结 `updated N, already-latest M, failed K`
4. 任一失败不影响其它继续；最终退出码：全成功 0，有失败非 0

### 9.4 remove 行为

1. config 命中已装？否 → `error: <name> not installed`，退出非 0
2. 删除 `~/.local/share/dyyl/plugins/<name>/` 整个插件目录（含所有版本）
3. 从 config.toml 删 `installed_plugins.<name>` 记录
4. 输出：`removed migpt`
5. 不删除当前已加载到运行中的进程的插件（remove 是离线操作；运行中脚本不受影响）

### 9.5 autoremove 行为

1. 遍历 config.installed_plugins
2. 对每个插件：若 `last_used_at` 距今 > 30 天，按 remove 流程卸载
3. `last_used_at` 缺失（旧版本安装的记录无此字段）→ 视为从未使用，立即清理
4. 输出：每个被清理的插件一行 `removed migpt (last used 45 days ago)`，最后总结 `autoremoved N plugins`
5. 退出码 0

**"30 天"阈值**：常量 `AUTOREMOVE_DAYS = 30`，硬编码 v1 不配置化。

### 9.6 使用时间戳追踪

`config.toml` 的 `installed_plugins.<name>` 记录新增 `last_used_at` 字段：

```toml
[installed_plugins.migpt]
version = "0.1.0"
installed_at = "2026-07-13T10:30:00Z"
last_used_at = "2026-07-13T14:20:00Z"   # 每次脚本运行中调用该插件时更新
sha256 = "abc..."
```

**更新时机**：脚本运行时，PluginManager 首次成功加载某插件 handle 后，写 `last_used_at = now()` 到 config。同一脚本内多次调用同一插件只更新一次（首次加载时）。

**时间格式**：RFC 3339 UTC（与 `installed_at` 一致），用 `chrono` crate（已是依赖）。

### 9.7 list 行为

输出表格：

```
NAME        VERSION   LAST USED         INSTALLED
migpt       0.1.0     2026-07-13 14:20  2026-07-13 10:30
example     0.2.0     2026-07-10 09:00  2026-07-08 18:00
```

`last_used_at` 缺失显示 `-`。退出码 0。

### 9.8 CLI 解析逻辑（main.rs 改动）

```rust
// 伪代码
let first = args.get(1);
match first.map(String::as_str) {
    Some("install") if args.len() == 3 => cli_plugin_install(&args[2], lang),
    Some("update") if args.len() == 2 => cli_plugin_update_all(lang),
    Some("update") if args.len() == 3 => cli_plugin_update_one(&args[2], lang),
    Some("remove") if args.len() == 3 => cli_plugin_remove(&args[2], lang),
    Some("autoremove") if args.len() == 2 => cli_plugin_autoremove(lang),
    Some("list") if args.len() == 2 => cli_plugin_list(lang),
    _ => { /* 现有 --flag <filename> 逻辑 */ }
}
```

`--lang` 等全局选项需在子命令之前，例如 `dyyl --lang zh install migpt`。

### 9.9 与脚本运行的关系

CLI 子命令是**离线管理**操作，不运行脚本，不 dlopen 插件。它们只操作文件系统与 config。

脚本运行时的"首次调用惰性下载"（§3.3）仍保留——即使没显式 `install`，脚本调用未装插件也会自动下载。两条路径共享同一套 fetch/install/store 实现（§10 实现顺序的 `fetch.rs` + `store.rs` 被 CLI 与运行时共同调用）。

---

## 10. 测试策略

### 10.1 测试分层

| 层 | 目的 | 工具 |
|---|---|---|
| 单元（runtime/plugin/*） | 各模块独立逻辑 | `cargo test` |
| 集成（端到端调用） | 真实插件被加载、调度、返回 | `cargo test` + 测试用 cdylib fixture |
| 协议（manifest/SHA256） | 解析、校验、平台选择 | 单元 + JSON fixture |
| 错误模型 | 与内置 RuntimeError 对齐 | 单元 + golden fixture |
| 迁移对称性 | 验证"内置→插件几乎无差别" | 对照测试 |
| CLI 子命令 | install/update/remove/autoremove/list | `cargo test` + 本地 HTTP server + tempdir |

### 10.2 测试用插件 fixture

`tests/fixtures/plugins/example/` 下放一个最小 Rust cdylib crate，导出 14 个 ABI 符号。`handle_command` 实现三个命令：

- `echo(x)` → 原样返回 x（验证 Value 全类型来回无损）
- `addone(n)` → n+1（验证数值路径）
- `math.double(n)` → n*2（**多级子命令**，验证 cmd_name 含点号时路由正确）

构建脚本 `tests/fixtures/plugins/example/build.sh` 在测试前 `cargo build --release`，产物 `libexample.so` 放到 tmpdir。集成测试 `dlopen` 它。

### 10.3 关键测试用例（最少集）

**manifest.rs**

- 解析合法 manifest.json ✓
- 缺字段 / 类型错 → 错误
- abi_version 不匹配 → 拒绝
- dyyl_min 不满足 → 拒绝
- 当前平台无条目 → 拒绝
- 多平台条目正确选当前平台
- commands[].name 含点号（多级）正确解析

**fetch.rs**（用本地 HTTP server + tempfile，不打外网；路径结构严格遵循 §8.2）

- 下载产物到临时文件 ✓
- SHA256 不符 → 拒绝并清理
- 网络失败 → RuntimeError
- 产物写入最终路径并落 plugin.toml 副本

**loader.rs**

- dlopen 成功 + 14 符号全解析 ✓
- 缺符号 → 错误
- `plugin_init` 返回 NULL → 错误
- `on_load` 返回非 0 → 拒绝
- `list_commands` JSON 解析 + arity 校验（含多级 name）
- `handle_command` 成功路径：args JSON → out Value
- `handle_command` 返回非 0：解析 error JSON → RuntimeError
- `free_string` 跨分配器安全（插件 malloc / dyyl 调 free_string 让插件 free）

**dispatch 集成**

- `math.add` 等已知族仍走内置路径（不误判为插件）✓
- 未知前缀 `<name>.<sub>` 触发插件路径
- **多级命令 `example.math.double` 正确切分**：plugin_name="example", cmd_name="math.double"（注意：虽然 `math.` 是已知族前缀，但完整命令 `example.math.double` 的首段是 `example`，非已知族，走插件路径）
- 首次调用触发下载+加载+调用一条龙
- 第二次调用复用已加载 handle（无再 dlopen）
- 插件调用失败 → sentinel + 脚本继续
- panic_mode=abort 的插件文档警告（无法在测试里安全触发真 panic，仅测配置识别）

**Value 编解码对称性**（迁移对称性核心）

- 每种 Value 类型：Num/Str/Expr/Empty/List/Dict 各往返一次，断言相等
- 嵌套结构（List of Dict of Num）往返
- `echo` 命令对所有类型 fixture 跑一遍

**错误对象对齐**

- error JSON 含 code/message → 转 RuntimeError 后字段对齐
- sentinel 与内置命令失败 sentinel 同形
- golden fixture `tests/fixtures/plugin-error.dyyl` 验证 stderr 输出格式

**CLI 子命令**（用本地 HTTP server + tempdir 模拟 HOME 与 l.dyyapp.com）

- `dyyl install migpt` → 产物落到 tempdir 的 XDG data，config 写入记录，退出 0
- `dyyl install migpt`（已装同版本）→ 输出 `already installed`，退出 0，不重复下载
- `dyyl install migpt`（已装旧版本）→ 升级到新版本，输出 `updated`
- `dyyl install unknown`（manifest 404）→ 输出错误，退出非 0
- `dyyl update migpt`（已是 latest）→ `already latest`，退出 0
- `dyyl update migpt`（有新版本）→ 升级，退出 0
- `dyyl update migpt`（未装）→ 错误，退出非 0
- `dyyl update`（多个已装，混合 already/latest/failed）→ 汇总输出正确，退出码符合 K>0 时非 0
- `dyyl remove migpt` → 目录删除 + config 记录删除，退出 0
- `dyyl remove unknown`（未装）→ 错误，退出非 0
- `dyyl autoremove`（有 > 30 天、有 < 30 天、有 last_used_at 缺失三种）→ 只清理前两种 + 缺失的，输出天数
- `dyyl list`（空、有插件、有缺失 last_used_at 的）→ 表格输出正确
- `last_used_at` 更新：脚本运行中首次加载插件后 config 记录被刷新（用 mock 时间或检查时间戳非空）
- `--lang zh install migpt` → 子命令前全局选项生效，输出中文

**i18n 双语断言**（详见 §12.10）

- 覆盖率 gate：`missing_translations(En/Zh)` 必须空
- 抽样键 en/zh 返回不同字符串
- 多参数插值正确
- fallback：zh 缺失回退 en + warning（去重）
- 插件注册后查插件键命中
- 运行时插件错误路径在 `--lang zh/en` 下输出对应语言
- 插件返回的 error.message 不被翻译

### 10.4 不测的（明确边界）

- 不测真 panic 跨 FFI UB（无法安全复现，仅文档警告）
- 不测外网 `l.dyyapp.com`（用本地 HTTP server 替代）
- 不测 Windows/macOS 实际加载（CI 仅 Linux，其它平台靠平台条目选择逻辑单测覆盖）
- 不测 cdylib 跨 Rust 版本兼容（文档声明 ABI 锁定，dyyl 升 API 版本时才允许破坏）
- 不测 autoremove 的真实 30 天等待（用直接构造过期 last_used_at 的 config fixture）

### 10.5 与现有测试套件集成

- `cargo test` 一键跑全部，包括新插件测试
- 新增 `tests/plugin_tests.rs` 集成测试入口（含运行时与 CLI 两组用例）
- 新增 `tests/cli_plugin_tests.rs` 或合入 `plugin_tests.rs`，专测 CLI 子命令
- 新增 `tests/fixtures/plugin-*.dyyl` golden 脚本
- `cargo fmt --check` + `cargo clippy --all-targets --all-features` 必须通过（项目 lint 严格，`unwrap_used`/`panic`/`indexing_slicing` 全 deny，插件代码与 CLI 代码也需遵守）

---

## 11. 实现顺序（高层，详细计划由 writing-plans 产出）

**阶段 0：i18n 基础设施重设计（前置，必须先完成）**

1. 新建 `locales/en.json` + `locales/zh.json`，把现有所有手写消息迁成键值
2. 重写 `src/i18n.rs`：`MessageStore` + `t(lang, key, args)` + `register_plugin()` + `all_keys()` + `missing_translations()`
3. 现有所有 `pub fn` 改为薄包装调 `t()`（调用点暂不动）
4. 测试覆盖率 gate：`missing_translations(En/Zh)` 必须空
5. CI 加 i18n 覆盖率断言

**阶段 1：插件运行时**

6. ABI 类型与签名（`abi.rs`）——纯类型，无副作用
7. manifest 解析（`manifest.rs`）——纯解析，可单测
8. store 路径管理（`store.rs`）——纯路径计算
9. registry（`registry.rs`）——扫已装目录
10. loader（`loader.rs`）——dlopen + 调用，需测试 fixture
11. fetch（`fetch.rs`）——HTTP + SHA256，用本地 server 测；**被运行时与 CLI 共用**
12. PluginManager（`mod.rs`）——编排上述模块；首次加载成功后写 `last_used_at`；加载时调 `register_plugin` 注册插件 i18n
13. dispatch fallback（`cmd/plugin.rs` + 改 dispatch.rs）——含多级命令切分
14. config 扩展（`config.rs`）——增 `installed_plugins` + `last_used_at` 字段

**阶段 2：CLI 与服务器**

15. CLI 子命令（`src/cli/plugin_cmds.rs` + 改 `main.rs`）——install/update/remove/autoremove/list，复用 fetch/store
16. 测试 fixture 与集成测试（含多级命令、CLI 子命令、i18n 双语断言用例）
17. server.js 扩展（新增 `/plugins/...` 路由）
18. 发布脚本 `scripts/publish-plugin.sh`
19. 文档（README + dyyl-api-reference.md 增"插件系统"章节，含 UB 风险警告、多级命令说明、CLI 子命令用法、i18n 约定）

---

## 12. 双语适配（i18n 基础设施重设计）

### 12.1 背景与重设计动机

当前 [src/i18n.rs](file:///workspace/src/i18n.rs) 采用"每条消息一个手写 `pub fn` + `match lang` 返回 `&'static str`"风格。这种方式有以下根本问题，无法支撑插件生态：

1. **无键值分离**：文案硬编码在函数体，无法外部化、无法审计覆盖率
2. **无统一插值**：`format!` 散落各处，参数语序/复数无法处理
3. **无 fallback**：zh 缺某条直接编译/运行出错，无回退链
4. **无法被插件复用**：插件作者若想做双语，得自己从零造一套
5. **无法批量测试**：要逐函数断言，新增一条消息改两处（en/zh），易漏
6. **与"内置命令迁移到插件"冲突**：内置命令的错误消息若都是手写函数，迁移时无法打包带走

**v1 重设计为**：自研轻量键值表 + JSON 资源文件 + zh 缺失回退 en + 插件自带 JSON 注册到 dyyl。无新依赖（serde_json 已是依赖），够用即可，不支持 ICU 复数/性别（错误消息用不到）。

### 12.2 模块结构

```
src/i18n.rs       — MessageStore + t() + register_plugin() + 覆盖率检查 API
locales/
  en.json         — 英文消息资源（编译进二进制 include_str!）
  zh.json         — 中文消息资源（编译进二进制 include_str!）
```

资源文件编译期 `include_str!` 嵌入二进制，**运行时不读磁盘**（避免部署时缺文件）。开发期改 JSON 需 `cargo build` 重编译——这是有意为之，避免运行时热重载引入的复杂度与潜在不一致。

### 12.3 数据结构

```rust
pub struct MessageStore {
    en: HashMap<String, String>,   // key -> template
    zh: HashMap<String, String>,
    plugins: HashMap<String, PluginMessages>,  // plugin_name -> 该插件的 en/zh 表
}

pub struct PluginMessages {
    en: HashMap<String, String>,
    zh: HashMap<String, String>,
}
```

### 12.4 核心 API

```rust
/// 查键 + 插值。args 为 (&str, &str) 参数对。
/// 查找顺序：插件表（若 key 以 "<plugin_name>." 开头）→ dyyl 主表 →
///           fallback：zh 缺失用 en + eprintln warning（仅一次/键）
pub fn t(lang: Lang, key: &str, args: &[(&str, &str)]) -> String;

/// 插件注册自己的 en/zh JSON。在 PluginManager 加载插件时调用。
/// path 指向插件包内 locales/ 目录。
pub fn register_plugin(store: &mut MessageStore, name: &str, en_path: &Path, zh_path: &Path) -> Result<()>;

/// 列出 dyyl 主表所有键（测试覆盖率用）
pub fn all_keys() -> Vec<&'static str>;

/// 列出某语言下缺失的键（CI gate：必须为空）
pub fn missing_translations(lang: Lang) -> Vec<&'static str>;
```

**插值约定**：模板用 `{name}` 占位符，运行时按 `args` 替换。不支持嵌套、不支持复数——错误消息不需要。若需复杂格式，调用方先 `format!` 再传完整字符串当单个 `arg`。

### 12.5 资源文件格式

```jsonc
// locales/en.json
{
  "cli.version_banner": "dyyl 0.2.0 — script interpreter",
  "cli.usage": "Usage: dyyl [--debug] [--lang <en|zh>] <filename>",
  "runtime.unknown_command": "unknown command: {command}",
  "mcm.no_host_provider": "no mcm host provider for: {name}",

  "plugin.install_success": "installed {name} {ver}",
  "plugin.already_installed": "{name} {ver} already installed",
  "plugin.updated": "updated {name} {old} -> {new}",
  "plugin.already_latest": "{name} already latest ({ver})",
  "plugin.removed": "removed {name}",
  "plugin.not_installed": "error: {name} not installed",
  "plugin.install_failed": "error: failed to install {name}: {reason}",
  "plugin.update_failed": "error: failed to update {name}: {reason}",
  "plugin.remove_failed": "error: failed to remove {name}: {reason}",
  "plugin.update_all_summary": "updated {updated}, already-latest {latest}, failed {failed}",
  "plugin.autoremove_summary": "autoremoved {count} plugins",
  "plugin.autoremove_removed": "removed {name} (last used {days_ago} days ago)",
  "plugin.list_header": "NAME VERSION LAST_USED INSTALLED",
  "plugin.list_empty": "no plugins installed",

  "plugin.fetch_manifest_failed": "failed to fetch manifest for '{name}': {reason}",
  "plugin.manifest_not_found": "plugin '{name}' not found on l.dyyapp.com",
  "plugin.abi_mismatch": "plugin '{name}' ABI version mismatch: expected {expected}, got {actual}",
  "plugin.dyyl_min_unmet": "plugin '{name}' requires dyyl >= {required}, current is {current}",
  "plugin.platform_unavailable": "plugin '{name}' has no build for {platform}; available: {available}",
  "plugin.sha256_mismatch": "plugin '{name}' SHA256 checksum mismatch",
  "plugin.download_failed": "failed to download plugin '{name}': {reason}",
  "plugin.dlopen_failed": "failed to load plugin '{name}': {reason}",
  "plugin.symbol_missing": "plugin '{name}' missing required symbol '{symbol}'",
  "plugin.init_failed": "plugin '{name}' init() returned NULL",
  "plugin.on_load_failed": "plugin '{name}' on_load() failed with code {code}",
  "plugin.unknown_subcommand": "plugin '{name}' has no command '{sub}'",
  "plugin.arity_mismatch": "plugin command '{name}.{sub}' expects {expected} args, got {actual}",
  "plugin.command_failed": "plugin command '{name}.{sub}' failed: {code}",
  "plugin.panic_warning": "warning: plugin '{name}' panicked; behavior is undefined (panic_mode not isolated)",

  "cli.plugin_usage": "Usage: dyyl install <name> | update [name] | remove <name> | autoremove | list",
  "cli.plugin_subcommand_unknown": "dyyl: unknown subcommand '{sub}'"
}
```

```jsonc
// locales/zh.json —— 同键，中文文案
{
  "cli.version_banner": "dyyl 0.2.0 — 脚本解释器",
  "cli.usage": "用法: dyyl [--debug] [--lang <en|zh>] <文件名>",
  "runtime.unknown_command": "未知命令: {command}",
  "mcm.no_host_provider": "无 mcm host provider: {name}",

  "plugin.install_success": "已安装 {name} {ver}",
  "plugin.already_installed": "{name} {ver} 已安装",
  "plugin.updated": "已升级 {name} {old} -> {new}",
  "plugin.already_latest": "{name} 已是最新 ({ver})",
  "plugin.removed": "已卸载 {name}",
  "plugin.not_installed": "错误: {name} 未安装",
  "plugin.install_failed": "错误: 安装 {name} 失败: {reason}",
  "plugin.update_failed": "错误: 升级 {name} 失败: {reason}",
  "plugin.remove_failed": "错误: 卸载 {name} 失败: {reason}",
  "plugin.update_all_summary": "已升级 {updated}, 已最新 {latest}, 失败 {failed}",
  "plugin.autoremove_summary": "自动清理了 {count} 个插件",
  "plugin.autoremove_removed": "已卸载 {name} (上次使用 {days_ago} 天前)",
  "plugin.list_header": "名称 版本 上次使用 安装时间",
  "plugin.list_empty": "未安装任何插件",

  "plugin.fetch_manifest_failed": "获取 '{name}' 清单失败: {reason}",
  "plugin.manifest_not_found": "l.dyyapp.com 上未找到插件 '{name}'",
  "plugin.abi_mismatch": "插件 '{name}' ABI 版本不匹配: 期望 {expected}, 实际 {actual}",
  "plugin.dyyl_min_unmet": "插件 '{name}' 需要 dyyl >= {required}, 当前 {current}",
  "plugin.platform_unavailable": "插件 '{name}' 无 {platform} 构建; 可用: {available}",
  "plugin.sha256_mismatch": "插件 '{name}' SHA256 校验和不匹配",
  "plugin.download_failed": "下载插件 '{name}' 失败: {reason}",
  "plugin.dlopen_failed": "加载插件 '{name}' 失败: {reason}",
  "plugin.symbol_missing": "插件 '{name}' 缺少必需符号 '{symbol}'",
  "plugin.init_failed": "插件 '{name}' init() 返回 NULL",
  "plugin.on_load_failed": "插件 '{name}' on_load() 失败，码 {code}",
  "plugin.unknown_subcommand": "插件 '{name}' 无命令 '{sub}'",
  "plugin.arity_mismatch": "插件命令 '{name}.{sub}' 期望 {expected} 个参数, 实际 {actual}",
  "plugin.command_failed": "插件命令 '{name}.{sub}' 失败: {code}",
  "plugin.panic_warning": "警告: 插件 '{name}' panic; 行为未定义 (panic_mode 未隔离)",

  "cli.plugin_usage": "用法: dyyl install <名称> | update [名称] | remove <名称> | autoremove | list",
  "cli.plugin_subcommand_unknown": "dyyl: 未知子命令 '{sub}'"
}
```

### 12.6 键命名约定

- **命名空间.动作**：`plugin.sha256_mismatch`、`cli.usage`、`runtime.unknown_command`、`mcm.no_host_provider`
- **插件键前缀**：`<plugin_name>.<key>`，如 `migpt.greet.brief`、`migpt.user.login.error`
- 插件键与 dyyl 主键不冲突（前缀即插件名，dyyl 主键无 `migpt.` 这种前缀）
- 键名全小写 + 下划线分隔，不用驼峰

### 12.7 消息归属边界

| 消息来源 | 翻译责任 | 走 i18n？ |
|---|---|---|
| dyyl 自生消息（manifest/SHA256/网络/ABI/路由/CLI 输出/list 摘要） | dyyl | 是，必须走 `t(lang, "plugin.xxx", args)` |
| 插件返回的 error 对象 `message` 字段 | 插件作者 | **否**，dyyl 透传原文（插件若做了双语，自己选 lang 返回对应文案） |
| 插件 `list_commands` 输出的 `brief` 字段 | 插件作者 | **否**，透传原文 |
| 插件 `get_command_help` 输出 | 插件作者 | **否**，透传原文 |
| 错误对象 `code` 枚举 | 机器可读 | 否 |

**注意**：插件作者的 `message`/`brief`/`help` 若要双语，**自己在插件内处理**——v1 ABI 的 `plugin_init(api_version)` 不传 lang（见 §13 开放问题）。dyyl 不会代为翻译插件返回的字符串。

### 12.8 插件 i18n（插件作者侧）

插件包结构：

```
~/.local/share/dyyl/plugins/migpt/0.1.0/
  libmigpt.so
  plugin.toml
  locales/
    en.json   # {"migpt.greet.brief": "Send a greeting", "migpt.user.login.brief": "User login"}
    zh.json   # {"migpt.greet.brief": "发送问候", "migpt.user.login.brief": "用户登录"}
```

**加载流程**：PluginManager 在 `dlopen` + `plugin_init` 成功后，检查插件目录下是否有 `locales/en.json` + `locales/zh.json`：

- 都有 → 调 `register_plugin(store, "migpt", en_path, zh_path)`
- 只有 en.json → 注册 en，zh 缺失时 fallback 到 en（与 dyyl 主表一致）
- 都没有 → 跳过注册，插件键查表时直接 fallback 到 dyyl 主表（若主表也无此键，返回键名本身 + warning）

**插件键查表优先级**：`t(lang, "migpt.greet.brief", args)` 查找顺序：

1. 插件表 `migpt` 的 zh/en
2. dyyl 主表的 zh/en
3. fallback：返回键名本身 `"migpt.greet.brief"` + eprintln warning

**插件 manifest 声明**：远程 manifest.json 可选 `has_locales: true` 字段，提示 dyyl 该插件带 locales 目录。无此字段则 dyyl 仍会检查目录是否存在（兜底）。

### 12.9 现有消息迁移

现有 `src/i18n.rs` 的所有 `pub fn`（如 `cli_version_banner`、`unknown_command`、`mcm_no_host_provider` 等）迁移为：

```rust
// 迁移前
pub fn cli_version_banner(lang: Lang, ver: &str) -> String {
    match lang {
        Lang::En => format!("dyyl {} — script interpreter", ver),
        Lang::Zh => format!("dyyl {} — 脚本解释器", ver),
    }
}

// 迁移后（薄包装，保留旧 API 不破坏调用点）
pub fn cli_version_banner(lang: Lang, ver: &str) -> String {
    t(lang, "cli.version_banner", &[("ver", ver)])
}
```

**迁移策略**：保留所有现有 `pub fn` 作为薄包装调 `t()`，调用点（main.rs、runtime/*）暂不改——这样迁移工作集中在 i18n.rs 内部，不波及整个代码库。后续可渐进把调用点直接改为 `t()` 调用，删掉薄包装。新代码（插件相关）直接用 `t()`，不再加薄包装。

### 12.10 测试覆盖（i18n 专项）

在 `tests/i18n_tests.rs` 现有基础上扩展：

**覆盖率 gate（CI 必须通过）**

- `missing_translations(Lang::En)` 返回空 Vec
- `missing_translations(Lang::Zh)` 返回空 Vec
- `all_keys()` 非空（防止资源文件加载失败导致空表）

**双语断言**

- 抽样若干键，在 `Lang::En` 与 `Lang::Zh` 下返回**不同**字符串（防止误返回同一字面量，如 zh.json 误填了英文）
- `t(En, "plugin.sha256_mismatch", &[("name","migpt")])` == `"plugin 'migpt' SHA256 checksum mismatch"`
- `t(Zh, "plugin.sha256_mismatch", &[("name","migpt")])` == `"插件 'migpt' SHA256 校验和不匹配"`

**插值正确性**

- 多参数插值：`t(En, "plugin.updated", &[("name","migpt"),("old","0.1.0"),("new","0.2.0")])` == `"updated migpt 0.1.0 -> 0.2.0"`
- 缺参数：模板里 `{x}` 但 args 没传 → 保留 `{x}` 原样 + eprintln warning（不 panic）

**fallback 行为**

- 模拟 zh 缺某键：`t(Zh, "test.missing_key", &[])` 返回 en 原文 + stderr 出现一次 warning
- 重复查同一缺失键：warning 只出现一次（去重）
- 插件键缺失：`t(En, "unknown_plugin.some_key", &[])` 返回键名本身 + warning

**插件注册**

- `register_plugin` 加载合法 en/zh JSON → 之后 `t(Zh, "migpt.greet.brief", &[])` 命中插件表
- 只有 en.json → zh 查不到时 fallback 到插件 en
- 都没有 → 不报错，查插件键时 fallback 到 dyyl 主表

**集成路径**

- 运行时插件错误路径在 `--lang zh` 下输出中文（manifest 404、SHA256 不符、ABI 不匹配、缺符号、arity 错）
- 同一路径在 `--lang en` 下输出英文
- 插件返回的 error.message 在 en/zh 下都显示插件原文（不被翻译）
- `dyyl --lang zh list` 表头 `名称 版本 上次使用 安装时间`
- `dyyl --lang en list` 表头 `NAME VERSION LAST_USED INSTALLED`
- `dyyl --lang zh autoremove` 摘要 `自动清理了 N 个插件`
- `dyyl --lang zh update` 摘要 `已升级 N, 已最新 M, 失败 K`

### 12.11 `--debug` 诊断

插件调用失败时若 `--debug` 开启，stderr 输出格式与现有 `debug_diagnostic` 一致（line / command / reason 三段），reason 经 `t()` 走 i18n。

---

## 13. 开放问题（实现时再决）

- `plugin.help`、`plugin.search` 等脚本内管理命令是否 v1 实现？当前 spec 不包含，CLI 子命令已覆盖管理需求。
- 升级时旧版本目录是否自动清理？v1 保留（remove 是整插件目录全删；update 旧版本保留以备回滚），后续可加 GC。
- `dyyl_min` 的版本比较是 SemVer 严格比较还是字符串比较？实现时选 SemVer（引入 `semver` crate）。
- 多级命令的 `cmd_name` 传给插件时是否包含插件名前缀？当前 spec 决定**不包含**（插件收到的就是 `user.login` 而非 `migpt.user.login`）。若插件需要完整路径可从 `on_load` 时收到的元数据自取，但 v1 不传。
- 插件名是否允许含点号？当前 spec 假设插件名是单段标识符（无点号），首个点号即分隔符。若未来需支持带点号的插件名，需引入转义或显式声明机制——v1 不支持。
- `dyyl update` 是否应并行下载多个插件？v1 串行实现简单，后续可加并发。
- `autoremove` 的 30 天阈值是否可配置？v1 硬编码，未来可加 `dyyl config autoremove.days N`。
- `dyyl install` 是否支持一次装多个（`dyyl install a b c`）？v1 只支持单个，后续可扩展。
- 插件是否需要知道当前 `lang`？v1 ABI 的 `plugin_init(api_version)` 不传 lang，插件无法做双语；后续可加 `init(api_version, lang)` 或环境变量。
- `dyyl list` 是否输出彩色（带 ANSI）？v1 纯文本，后续可加 `--color` 选项。
- i18n 资源文件是否需要支持用户自定义覆盖（如 `~/.config/dyyl/locales/zh.json` 覆盖内置翻译）？v1 不支持，内置 `include_str!` 编译期嵌入；后续若需求出现可加运行时覆盖层。
- 插件 locales 是否支持除 en/zh 外的语言？v1 只 en/zh 与 dyyl 自身一致；后续若 dyyl 加第三语言同步扩展。
