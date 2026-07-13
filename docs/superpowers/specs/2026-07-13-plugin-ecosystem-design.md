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

### 1.2 非目标（v1）

- 不支持第三方源；不开放任意 URL 下载。
- 不做插件签名验证（PKI）；仅 HTTPS + SHA256 清单校验。
- 不做插件 panic 跨 FFI 隔离；仅文档警告 UB 风险。
- 不做版本锁定 / 锁文件；始终使用 latest。
- 不做权限/能力沙箱；插件拥有 dyyl 同等权限。
- 不实现 `plugin.install` 显式命令或脚本头声明语法。

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
  fetch.rs        — 从 l.dyyapp.com 拉清单+产物+SHA256 校验
  store.rs        — 存储路径管理（XDG data）
src/runtime/cmd/plugin.rs — 路由未知前缀到 PluginManager
```

### 3.2 改动点

- [src/runtime/cmd/dispatch.rs](file:///workspace/src/runtime/cmd/dispatch.rs) 末尾 `_` 分支前加 fallback：命令名含至少一个点号且首段非已知命令族时，按首个点号切分出 `plugin_name` 与 `sub`（可含点号），转交 `plugin::dispatch(plugin_name, sub, call, env, ctx)`。
- [src/runtime/env.rs](file:///workspace/src/runtime/env.rs) 的 `Env` 增 `plugin_manager: PluginManager` 字段。
- [src/config.rs](file:///workspace/src/config.rs) 增 `installed_plugins: HashMap<String, InstalledPluginRecord>` 跟踪已装版本，便于将来诊断与升级提示。

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

## 9. 测试策略

### 9.1 测试分层

| 层 | 目的 | 工具 |
|---|---|---|
| 单元（runtime/plugin/*） | 各模块独立逻辑 | `cargo test` |
| 集成（端到端调用） | 真实插件被加载、调度、返回 | `cargo test` + 测试用 cdylib fixture |
| 协议（manifest/SHA256） | 解析、校验、平台选择 | 单元 + JSON fixture |
| 错误模型 | 与内置 RuntimeError 对齐 | 单元 + golden fixture |
| 迁移对称性 | 验证"内置→插件几乎无差别" | 对照测试 |

### 9.2 测试用插件 fixture

`tests/fixtures/plugins/example/` 下放一个最小 Rust cdylib crate，导出 14 个 ABI 符号。`handle_command` 实现三个命令：

- `echo(x)` → 原样返回 x（验证 Value 全类型来回无损）
- `addone(n)` → n+1（验证数值路径）
- `math.double(n)` → n*2（**多级子命令**，验证 cmd_name 含点号时路由正确）

构建脚本 `tests/fixtures/plugins/example/build.sh` 在测试前 `cargo build --release`，产物 `libexample.so` 放到 tmpdir。集成测试 `dlopen` 它。

### 9.3 关键测试用例（最少集）

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

### 9.4 不测的（明确边界）

- 不测真 panic 跨 FFI UB（无法安全复现，仅文档警告）
- 不测外网 `l.dyyapp.com`（用本地 HTTP server 替代）
- 不测 Windows/macOS 实际加载（CI 仅 Linux，其它平台靠平台条目选择逻辑单测覆盖）
- 不测 cdylib 跨 Rust 版本兼容（文档声明 ABI 锁定，dyyl 升 API 版本时才允许破坏）

### 9.5 与现有测试套件集成

- `cargo test` 一键跑全部，包括新插件测试
- 新增 `tests/plugin_tests.rs` 集成测试入口
- 新增 `tests/fixtures/plugin-*.dyyl` golden 脚本
- `cargo fmt --check` + `cargo clippy --all-targets --all-features` 必须通过（项目 lint 严格，`unwrap_used`/`panic`/`indexing_slicing` 全 deny，插件代码也需遵守）

---

## 10. 实现顺序（高层，详细计划由 writing-plans 产出）

1. ABI 类型与签名（`abi.rs`）——纯类型，无副作用
2. manifest 解析（`manifest.rs`）——纯解析，可单测
3. store 路径管理（`store.rs`）——纯路径计算
4. registry（`registry.rs`）——扫已装目录
5. loader（`loader.rs`）——dlopen + 调用，需测试 fixture
6. fetch（`fetch.rs`）——HTTP + SHA256，用本地 server 测
7. PluginManager（`mod.rs`）——编排上述模块
8. dispatch fallback（`cmd/plugin.rs` + 改 dispatch.rs）——含多级命令切分
9. config 扩展（`config.rs`）
10. 测试 fixture 与集成测试（含多级命令用例）
11. server.js 扩展（新增 `/plugins/...` 路由）
12. 发布脚本 `scripts/publish-plugin.sh`
13. 文档（README + dyyl-api-reference.md 增"插件系统"章节，含 UB 风险警告与多级命令说明）

---

## 11. 开放问题（实现时再决）

- `plugin.help`、`plugin.list`、`plugin.remove` 等管理命令是否 v1 实现？当前 spec 不包含，但 dispatch fallback 已留扩展点。
- 升级时旧版本目录是否自动清理？v1 可保留，避免回滚需求；后续可加 GC。
- `dyyl_min` 的版本比较是 SemVer 严格比较还是字符串比较？实现时选 SemVer（引入 `semver` crate）。
- 多级命令的 `cmd_name` 传给插件时是否包含插件名前缀？当前 spec 决定**不包含**（插件收到的就是 `user.login` 而非 `migpt.user.login`）。若插件需要完整路径可从 `on_load` 时收到的元数据自取，但 v1 不传。
- 插件名是否允许含点号？当前 spec 假设插件名是单段标识符（无点号），首个点号即分隔符。若未来需支持带点号的插件名，需引入转义或显式声明机制——v1 不支持。
