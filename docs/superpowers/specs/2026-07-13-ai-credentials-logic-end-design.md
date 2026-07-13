# dyyl AI 集成、凭证系统与 logic.end 设计

**日期：** 2026-07-13
**状态：** 设计已批准，待写实现计划
**关联：** dyyl v0.2.0；基于现有插件生态（[2026-07-13-plugin-ecosystem-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-plugin-ecosystem-design.md)）；ABI 从 v1 升到 v2

---

## 1. 目标与非目标

### 1.1 目标

- 在 dyyl 中加入 AI 集成：`ai.ask`（运行时同步请求 AI 返回字符串）与 `ai.auto`（源码占位符，执行前由 AI 批量填值并回写）。
- 加入通用凭证系统：服务 dyyl 内置 AI 凭证 + 插件自定义凭证，存储于 `~/.config/dyyl/credentials.toml`，缺失时交互式提示用户输入。
- 支持 AI Provider 三选一：`openai-chat`（Chat Completions）/ `openai-response`（Responses API）/ `anthropic`（Messages API），`base_url` 可选以支持兼容端点。
- 插件通过 manifest `credentials` 段声明所需凭证字段，dyyl 在 `on_load` 前预检并通过新 ABI 函数 `set_credentials` 注入。
- 加入 `logic.end` 命令：`logic.if/else/while/for` 的行数参数填 `_` 时进入"开放块"模式，由 `logic.end` 栈式关闭，支持嵌套。

### 1.2 非目标（v1）

- 不实现流式响应（SSE）：所有 AI 请求一次性返回完整响应。
- 不实现 AI 请求并发：多个 `ai.ask` 在脚本里顺序执行；多个 `ai.auto` 占位符合并为一次批量请求。
- 不实现凭证加密存储：明文 TOML，文件权限由用户负责（dyyl 不强制 chmod）。
- 不实现凭证轮换/过期检测：api_key 失效时返回哨兵或中止，由用户手动更新。
- 不实现 `ai.ask` 的工具调用（function calling）：纯文本响应。
- 不实现 `ai.auto` 的部分填充重试：批量请求失败则整体中止。
- 不修改现有插件 ABI v1 插件的兼容性：v1 插件无 `set_credentials` 符号时跳过注入，仍能加载。

---

## 2. 关键决策汇总

| # | 维度 | 决策 |
|---|---|---|
| 1 | `ai.ask` 参数 | 可变参数：`ai.ask <prompt>` 或 `ai.ask <system>, <prompt>`，`_` 跳过 system 用默认 |
| 2 | `ai.ask` 返回值 | 始终字符串（成功）/ `-1`（失败） |
| 3 | `ai.auto` 填值形式 | `ai.auto.filled <提示>, <值>` 两参数；提示可空（`_`） |
| 4 | `ai.auto` 触发 | 标记即状态：未填的 `ai.auto` 普通运行也请求；`dyyl build` 重置已填的重问 |
| 5 | 凭证存储位置 | `~/.config/dyyl/credentials.toml`（XDG config 目录） |
| 6 | 凭证文件结构 | 单文件多段：`[ai]` + `[plugin.<name>]` |
| 7 | 插件凭证声明 | manifest `credentials.fields` 段声明字段名/类型/secret/描述 |
| 8 | 凭证传递给插件 | 新 ABI `set_credentials(handle, json)`，`on_load` 前注入 |
| 9 | `logic.end` 嵌套 | 栈式关闭最近 `_` 块 |
| 10 | AI Provider | openai-chat / openai-response / anthropic，base_url 可选 |
| 11 | `ai.auto` system prompt | 内置 + 可被 `[ai].auto_system_prompt` 覆盖 |
| 12 | `dyyl build` 语义 | 只刷新（重置 + 重新填值）不执行 |
| 13 | 凭证提示流程 | 交互式逐字段（stderr 提示 + stdin 读） |
| 14 | AI 失败哨兵 | `ai.ask` 失败返回 `-1` |
| 15 | `[ai]` 段字段 | provider / api_key / model / base_url / auto_system_prompt（5 字段最小集） |
| 16 | 多 `ai.auto` 占位符 | 批量单请求 |
| 17 | 批量响应格式 | 编号 + JSON `{"1":{"type":"string","value":"..."}, ...}` |
| 18 | `logic.end` 适用 | if/else/while/for 全部支持 `_` 开放块 |
| 19 | 空提示定位 | `<<<AUTO_<id>: <hint or "(no hint, infer from position)">>>` 标记位置 |
| 20 | `ai.auto` 值类型 | AI 推断（JSON 原生类型区分 string/number） |
| 21 | HTTP 超时 | 1800 秒（30 分钟，适配长推理模型） |
| 22 | HTTP 重试 | 默认 3 次，指数退避（1s→2s→4s），仅重试网络错误/5xx/429 |
| 23 | 凭证文件权限 | 不自动 chmod 修正；新建按系统默认，已存在照原样；`--debug` 时权限过松仅警告 |
| 24 | 实现方案 | 方案 A：预扫描 + 运行时分离 |

---

## 3. 架构

### 3.1 新增模块

```
src/
  ai/
    mod.rs                       — AiProvider trait + dispatch 工厂
    provider_openai_chat.rs      — OpenAI Chat Completions API
    provider_openai_response.rs  — OpenAI Responses API
    provider_anthropic.rs        — Anthropic Messages API
    client.rs                    — HTTP 客户端（reqwest，含重试 3 次 + 超时 1800s）
    prompt.rs                    — ai.auto 批量 prompt 构造 + 响应解析
  credentials.rs                 — credentials.toml 读写 + 交互式提示
  prepass.rs                     — 执行前预扫描：检测 ai.auto 未填 → 批量请求 → 回写
  runtime/cmd/
    ai.rs                        — ai.ask / ai.auto.filled 命令 handler
  cli/
    mod.rs                       — 新增 build 子命令分发
```

### 3.2 改动点

- [src/runtime/cmd/dispatch.rs](file:///workspace/src/runtime/cmd/dispatch.rs)：新增 `ai.*` 命令族路由
- [src/runtime/cmd/mod.rs](file:///workspace/src/runtime/cmd/mod.rs)：注册 `ai` 模块
- [src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs)：`_` 行数 → 开放块模式 + `logic.end` 栈式关闭
- [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs)：执行前块边界预扫描，开放块栈管理
- [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs)：ABI v2，新增 `dyyl_plugin_set_credentials` 符号（共 15 个）
- [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs)：`RemoteManifest` + `LocalPluginToml` 增 `credentials` 段
- [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs)：`on_load` 前预检凭证 + 调 `set_credentials`
- [src/main.rs](file:///workspace/src/main.rs)：`build` 子命令分发 + 预扫描调用
- [src/lib.rs](file:///workspace/src/lib.rs)：注册 `ai` + `prepass` + `credentials` 模块
- [locales/en.json](file:///workspace/locales/en.json) + [locales/zh.json](file:///workspace/locales/zh.json)：新增 AI/凭证/logic.end 相关 i18n 键

### 3.3 数据流

```
dyyl <file>
  │
  ├─ main.rs 解析参数
  │
  ├─ prepass::run(file, env)
  │    ├─ 读文件文本
  │    ├─ 逐行扫描 ai.auto / ai.auto "提示"（未填的）
  │    ├─ 若有未填：
  │    │    ├─ credentials::ensure_ai() → 无凭证则交互提示
  │    │    ├─ prompt::build_batch(file_text, placeholders) → 构造批量请求
  │    │    ├─ ai::client::request(creds, system, user) → HTTP 调用（重试 3 次）
  │    │    ├─ prompt::parse_response(json) → {id: value} 映射
  │    │    └─ 回写 ai.auto.filled 到文件 → 落盘
  │    └─ 无未填 → 跳过
  │
  ├─ parser 解析（ai.auto.filled 当普通命令）
  │
  └─ runtime 执行
       ├─ ai.auto.filled "提示", 值 → 返回值（不请求 AI）
       └─ ai.ask [system], prompt → 运行时 HTTP → 字符串 / -1

dyyl build <file>
  │
  ├─ prepass::reset_filled(file) → 所有 ai.auto.filled → ai.auto
  └─ prepass::run(file, env) → 批量请求 + 回写（不执行脚本）
```

### 3.4 预扫描设计

预扫描只做**逐行文本扫描**找 `ai.auto` 模式（正则匹配），不做完整 parser 解析。原因：
- `ai.auto` 可能出现在任意位置（`set $x, ai.auto "提示"` / `file.write ai.auto "路径", ...`），完整解析需理解每行的命令结构
- 文本扫描足够定位占位符 + 提取提示词
- 回写时按行替换：把该行的 `ai.auto "提示"` 替换为 `ai.auto.filled "提示", 值`（或 `ai.auto` → `ai.auto.filled _, 值`）

---

## 4. `ai.ask` 与 `ai.auto` 命令语义

### 4.1 `ai.ask` — 运行时 AI 请求

**语法**（可变参数）：
```dyyl
# 单参数：用内置默认 system prompt
ai.ask <prompt>
ai.ask $prompt

# 双参数：自定义 system
ai.ask <system>, <prompt>

# 跳过 system 用默认
ai.ask _, <prompt>
```

**求值**：运行时同步 HTTP 请求 AI，返回字符串。

**示例**：
```dyyl
set $a, ai.ask "把 hello 翻译成法语"
io.out $a                          # 可能为 "Bonjour"

set $b, ai.ask "你是严格的翻译器，只输出译文", "把 hello 翻译成法语"
io.out $b

set $num_str, ai.ask "What is 2+2?"
set $num, str.to.num $num_str      # 用户显式转数值
```

**失败行为**：网络/认证/超时等失败（重试 3 次后仍失败）→ 返回 `-1`（数值哨兵）；`--debug` 时 stderr 输出错误详情（HTTP 状态码、错误消息、重试次数）。

**内置默认 system prompt**（`ai.ask` 单参数时）：
```
You are a helpful assistant. Answer the user's question concisely and accurately.
```

### 4.2 `ai.auto` — 源码占位符（执行前填充）

**源码形式**（两种）：
```dyyl
# 不带提示（AI 纯靠文件上下文 + <<<HERE>>> 标记推断）
set $name, ai.auto
file.write ai.auto, "content"

# 带提示
set $port, ai.auto "服务器端口，常用 25565"
```

**填值后形式**（预扫描回写）：
```dyyl
# 不带提示被填
set $name, ai.auto.filled _, "Steve"

# 带提示被填
set $port, ai.auto.filled "服务器端口，常用 25565", 25565
```

**`ai.auto.filled` 命令语义**：接受两参数（提示 + 值），运行时**不请求 AI**，直接返回值。提示参数仅为可追溯性保留，运行时忽略其内容。

### 4.3 值类型推断

批量请求的 JSON 响应里，值用 JSON 原生类型区分：
```json
{
  "1": {"type": "string", "value": "Steve"},
  "2": {"type": "number", "value": 25565},
  "3": {"type": "string", "value": "/var/log/app.log"}
}
```

回写时：
- `string` → 双引号包裹 + 转义特殊字符（`"` `\` 换行等，与现有引号规则一致）
- `number` → 裸数字字面量
- 运行时 `ai.auto.filled` 返回对应类型 Value（Str / Num / Expr）

### 4.4 批量请求构造

**预扫描发现 N 个未填 `ai.auto` 占位符后**，构造单次 AI 请求。

**system prompt**（内置，可被 `[ai].auto_system_prompt` 覆盖）：
```
You are filling placeholder values in a dyyl script. The user will give you
the full script content with placeholders marked. For each placeholder,
infer the appropriate value from context and the placeholder's hint.
Return ONLY a JSON object mapping placeholder IDs to {type, value}.
type is "string" or "number". Do not include any explanation.
```

**user prompt 结构**：
```
Below is a dyyl script with N placeholders marked as <<<AUTO_<id>: <hint or "(no hint, infer from position)">>>.

Replace each placeholder. Return JSON: {"1":{"type":"string","value":"..."}, "2":{"type":"number","value":42}, ...}

--- SCRIPT START ---
<file content with each ai.auto replaced by <<<AUTO_1: 端口常用25565>>>, ai.auto "提示" replaced by <<<AUTO_2: 提示>>>>
--- SCRIPT END ---
```

**示例**：

源码：
```dyyl
set $port, ai.auto "服务器端口，常用 25565"
set $name, ai.auto
file.write ai.auto "日志路径", "log"
```

构造的 user prompt 里脚本变为：
```dyyl
set $port, <<<AUTO_1: 服务器端口，常用 25565>>>
set $name, <<<AUTO_2: (no hint, infer from position)>>>
file.write <<<AUTO_3: 日志路径>>, "log"
```

AI 返回：
```json
{"1":{"type":"number","value":25565}, "2":{"type":"string","value":"Steve"}, "3":{"type":"string","value":"/var/log/app.log"}}
```

回写后源码：
```dyyl
set $port, ai.auto.filled "服务器端口，常用 25565", 25565
set $name, ai.auto.filled _, "Steve"
file.write ai.auto.filled "日志路径", "/var/log/app.log", "log"
```

### 4.5 失败行为

- 预扫描阶段批量请求失败（重试 3 次后仍失败）→ **中止脚本**，退出码 2。stderr 输出错误详情 + 重试次数，已填的占位符不回写（保持原状便于重试）。
- `dyyl build` 阶段失败 → 同上，退出码 2。
- 预扫描成功但个别占位符 AI 未返回（JSON 缺 ID） → 该占位符保持未填 `ai.auto`，stderr 警告，脚本继续（下次运行会再请求该占位符）。

---

## 5. 凭证系统

### 5.1 `credentials.toml` 结构

存储路径：`~/.config/dyyl/credentials.toml`（与 `config.toml` 同目录）。

```toml
# dyyl 内置 AI 凭证（ai.ask / ai.auto 用）
[ai]
provider = "openai-chat"          # openai-chat | openai-response | anthropic
api_key = "sk-..."
model = "gpt-4o-mini"
base_url = ""                     # 空=用官方端点；自定义则覆盖
auto_system_prompt = ""           # 空=用内置；非空则覆盖 ai.auto 的 system prompt

# 插件凭证（每个有 credentials 声明的插件一段）
[plugin.migpt]
token = "ghp_..."
repo = "foo/bar"

[plugin.other_plugin]
api_key = "abc..."
```

### 5.2 交互式提示流程

**触发时机**：
- AI：预扫描发现未填 `ai.auto` 且 `[ai]` 段缺失或不完整时；或首次 `ai.ask` 调用且 `[ai]` 缺失时。
- 插件：`dlopen` + `init` 成功后、`on_load` 前，检查 manifest 声明的 `credentials.fields` 是否在 `[plugin.<name>]` 段全部存在；缺失则触发提示。

**AI 凭证提示**（stderr 输出问题，stdin 读答案）：
```
[dyyl] AI 凭证未配置，请按提示输入：
  Provider (1=openai-chat, 2=openai-response, 3=anthropic): <用户输入 1>
  API Key: <用户输入>
  Model (如 gpt-4o-mini): <用户输入>
  Base URL (空=官方端点): <用户输入或空回车>
[dyyl] 凭证已保存到 ~/.config/dyyl/credentials.toml
```

写入后继续执行原 AI 请求。

**插件凭证提示**（按 manifest `credentials.fields` 声明的字段逐个问）：
```
[dyyl] 插件 'migpt' 需要凭证，请按提示输入：
  token (GitHub personal access token): <用户输入>
  repo (Default repository): <用户输入>
[dyyl] 凭证已保存
```

`secret: true` 的字段输入不回显（复用现有 `io.inpasswd` 机制）；`secret: false` 正常回显。

### 5.3 manifest `credentials` 段

扩展 [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs) 的 `RemoteManifest` + `LocalPluginToml`：

```jsonc
// remote manifest.json
{
  "name": "migpt",
  "commands": [...],
  "platforms": [...],
  "credentials": {
    "fields": [
      {"name": "token", "type": "string", "secret": true, "description": "GitHub personal access token"},
      {"name": "repo",  "type": "string", "secret": false, "description": "Default repository"}
    ]
  }
}
```

无 `credentials` 段 = 插件不需要凭证，跳过预检。

### 5.4 ABI v2：`set_credentials` 注入

[src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs) ABI 版本升到 2，新增第 15 个符号：

| 函数 | 签名 | 作用 |
|---|---|---|
| `dyyl_plugin_set_credentials` | `(handle, credentials_json) -> int` | dyyl 在 `on_load` 前调用，传插件凭证 JSON。返回 0=ok，非 0=插件拒绝（此时 dyyl 拒绝加载该插件）。 |

**调用顺序**（ABI v2）：
```
init(api_version) → set_credentials(handle, json) → on_load(handle) → handle_command* → on_unload(handle) → shutdown(handle)
```

**credentials_json 示例**：
```json
{"token": "ghp_...", "repo": "foo/bar"}
```
字段即 manifest 声明的 fields，值从 `credentials.toml` 的 `[plugin.<name>]` 段取。

**兼容性**：
- ABI v1 插件（无 `set_credentials` 符号）→ dyyl 检测符号缺失时跳过注入，插件无凭证可用（仍能加载，但插件自行决定是否报错）。
- ABI v2 插件必须实现 `set_credentials`（即使是空实现返回 0）。
- `DYRL_API_VERSION` 从 1 升到 2。

### 5.5 预检逻辑

[src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs) 加载流程扩展：

```
1. dlopen + 解析 15 个符号（v2）或 14 个（v1，回退）
2. plugin_init(api_version) → handle
3. 读 manifest.credentials.fields（若存在）
4. 读 credentials.toml [plugin.<name>]
5. 比对：fields 中每个 name 是否在 [plugin.<name>] 段存在
   - 全存在 → 构造 credentials_json，调 set_credentials(handle, json)
   - 有缺失 → 交互式提示用户补齐 → 写 credentials.toml → 调 set_credentials
6. on_load(handle)
7. 后续 handle_command 调用
```

### 5.6 安全注意

- `credentials.toml` **不自动 chmod 修正权限**。新建文件按系统默认权限（umask 决定）；已存在文件照原样读写。
- `--debug` 时若文件权限非 0600，stderr 输出一次警告：`credentials.toml permissions are loose (mode <octal>), consider chmod 600`，但不中止。
- 不在 stderr/日志里输出 api_key 等敏感字段值（`--debug` 也只输出"凭证已加载"/"凭证缺失"等摘要）。
- `set_credentials` 传给插件的 JSON 在 dyyl 内存里，插件负责自己不泄漏（dyyl 无法强制）。

---

## 6. `logic.end` 与开放块

### 6.1 语法

`logic.if` / `logic.else` / `logic.while` / `logic.for` 的第二个参数（行数）填 `_` 时，进入**开放块模式**，由 `logic.end` 关闭：

```dyyl
# 开放块
logic.if logic.same($a, 1), _
  io.out "a is 1"
logic.end

# 嵌套开放块
logic.while logic.less($i, 10), _
  io.out $i
  logic.if logic.same($i, 5), _
    io.out "halfway"
  logic.end
  set $i, math.add($i, 1)
logic.end

# 显式行数与开放块可混用
logic.if cond, 3
  io.out "line 1"
  io.out "line 2"
  io.out "line 3"
logic.while cond, _
  io.out "loop"
logic.end
```

### 6.2 `logic.end` 语义

- **栈式关闭**：关闭最近一个未关闭的开放块。
- 无参数。
- 返回值：`1`（与现有 `logic.if/else` 返回 1/0 一致，无实际语义意义）。
- 出现 `logic.end` 但栈空（无未关闭块）→ 哨兵 `0` + debug 警告，该行当作无操作。

### 6.3 执行模型改动

[src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs) 当前逻辑：读行数参数 → 跳过 N 行。开放块模式需要**反向**：扫描到 `logic.end` 才知道块体范围。

**实现策略**：执行前先做一次**块边界预扫描**，把每个 `_` 块的 `end_idx` 算出来存到命令的元数据里，运行时按 `end_idx` 跳转（复用现有行数逻辑，只是行数改为动态计算）：

```
预扫描（语法分析阶段）：
  遍历所有命令，维护开放块栈
  遇 _ 块 → push
  遇 logic.end → pop，记录该块的 end_idx
  扫描结束栈非空 → 错误：unclosed block

执行阶段：
  逻辑命令的行数参数：
    - 显式数字 N → 跳 N 行（现有逻辑）
    - _ → 从元数据查 end_idx，等价行数 = end_idx - 当前idx - 1
  exec_block.rs 改动最小：只是行数来源不同
```

这样 [exec_block.rs](file:///workspace/src/runtime/exec_block.rs) 的核心逻辑（if/else/while/for 执行）几乎不变，只是 `body_lines` 的计算从"读 args[1]"改为"args[1] 是数字则用，是 `_` 则查预扫描结果"。

### 6.4 嵌套规则

- 开放块可嵌套开放块（栈式管理）。
- 开放块可嵌套显式行数块（显式块不计入开放块栈，因为它的范围已知）。
- 显式行数块可嵌套开放块（开放块在显式块的 N 行内，`logic.end` 必须在那 N 行内出现，否则显式块会跳过 `logic.end` 导致开放块未关闭）。
- 一个开放块必须由一个 `logic.end` 关闭，不能跨文件、跨显式块。

### 6.5 错误情况

| 情况 | 行为 |
|---|---|
| 开放块未关闭（文件结束栈非空） | 错误：`unclosed block at line <start>`，中止脚本，退出码 4 |
| `logic.end` 栈空 | 哨兵 `0` + debug 警告，该行无操作，继续执行 |
| 显式块 N 行内嵌套开放块但 `logic.end` 在 N 行外 | 显式块跳过 `logic.end`，开放块留在栈里 → 文件结束时报"unclosed block" |
| `logic.for` 用 `_` | 合法（for 的次数参数仍是 args[0]，行数参数填 `_`） |

### 6.6 与现有显式行数的兼容

- 显式行数行为完全不变（`logic.if cond, 3` 仍跳 3 行）。
- `_` 是新引入的开放块标记，与现有 `_` 占位符语义一致（"跳过这个参数位"），但在此上下文表示"行数待定，用 logic.end 关闭"。
- 现有所有脚本无需修改。

---

## 7. AI Provider 客户端

### 7.1 `AiProvider` trait

[src/ai/mod.rs](file:///workspace/src/ai/mod.rs) 定义统一 trait：

```rust
pub trait AiProvider {
    /// 发送一次 AI 请求，返回响应文本。
    /// system 可能为空（用 provider 默认），user 是用户 prompt。
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError>;
}

pub enum AiProviderKind {
    OpenaiChat,
    OpenaiResponse,
    Anthropic,
}

pub struct AiError {
    pub kind: AiErrorKind,
    pub message: String,
    pub status: Option<u16>,
}

pub enum AiErrorKind {
    Network,        // 连接/超时
    Auth,           // 401/403，api_key 无效
    RateLimit,      // 429
    ServerError,    // 5xx
    Parse,          // 响应 JSON 解析失败
    Other,
}
```

### 7.2 三种 Provider 实现

**1. `openai-chat`**（Chat Completions API）

端点：`{base_url}/chat/completions`（base_url 空 = `https://api.openai.com/v1`）

请求体：
```json
{
  "model": "gpt-4o-mini",
  "messages": [
    {"role": "system", "content": "<system>"},
    {"role": "user", "content": "<user>"}
  ]
}
```

响应解析：`choices[0].message.content`

**2. `openai-response`**（Responses API）

端点：`{base_url}/responses`

请求体：
```json
{
  "model": "gpt-4o-mini",
  "instructions": "<system>",
  "input": "<user>"
}
```

响应解析：`output[0].content[0].text`（按 OpenAI Responses API 文档解析，可能需遍历 `output` 数组）

**3. `anthropic`**（Messages API）

端点：`{base_url}/v1/messages`（base_url 空 = `https://api.anthropic.com`）

请求头：`x-api-key: <api_key>` + `anthropic-version: 2023-06-01`

请求体：
```json
{
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 4096,
  "system": "<system>",
  "messages": [
    {"role": "user", "content": "<user>"}
  ]
}
```

响应解析：`content[0].text`

### 7.3 客户端工厂

```rust
pub fn build_provider(creds: &AiCredentials) -> Box<dyn AiProvider> {
    match creds.provider {
        AiProviderKind::OpenaiChat => Box::new(OpenaiChatProvider::new(
            creds.api_key.clone(),
            creds.model.clone(),
            creds.base_url.clone(),
        )),
        AiProviderKind::OpenaiResponse => Box::new(OpenaiResponseProvider::new(...)),
        AiProviderKind::Anthropic => Box::new(AnthropicProvider::new(...)),
    }
}
```

### 7.4 HTTP 客户端

- 复用现有 `reqwest` 依赖。
- **超时：1800 秒**（30 分钟，适配长推理模型如 o1/o3）。
- **重试：默认 3 次**。指数退避（1s → 2s → 4s）。触发重试的条件：
  - 网络错误（连接超时、读超时、DNS 失败）
  - 5xx 服务端错误
  - 429 限流（按 `Retry-After` 头退避，无头则指数退避）
- 不重试：4xx（除 429）—— 认证错误、参数错误等不可恢复。
- TLS：reqwest 默认启用 rustls。
- 请求/响应不落盘日志（避免 api_key 泄漏）；`--debug` 时仅输出 `POST <url> -> <status>` 摘要 + 重试次数。

最坏情况耗时：1800s × 4 次请求 + 1s + 2s + 4s 退避 ≈ 7207s（约 2 小时）。这是有意的，因为某些 AI 推理任务确实需要很长时间。

### 7.5 system prompt 处理

- `ai.ask` 单参数 → system = 内置默认（`"You are a helpful assistant..."`）。
- `ai.ask` 双参数 → system = 用户传入。
- `ai.ask _, prompt` → system = 内置默认（`_` 跳过）。
- `ai.auto` 批量 → system = `[ai].auto_system_prompt`（空则用内置 ai.auto 专用 prompt，见第 4 节）。

若 system 为空字符串且 provider 是 Anthropic（Anthropic 要求 system 可选）→ 不传 system 字段。OpenAI Chat 则传空 system message。

### 7.6 响应解析容错

- AI 返回的 JSON 可能被包在 ```` ```json ... ``` ```` 里 → 剥离 markdown 代码块标记后再解析。
- AI 返回额外解释文本 → 提取第一个 `{` 到最后一个 `}` 之间的子串再解析。
- 解析失败 → `AiError::Parse`，预扫描中止。

---

## 8. CLI 子命令与预扫描

### 8.1 CLI 解析扩展

[src/main.rs](file:///workspace/src/main.rs) 新增 `build` 子命令：

| 调用 | 行为 |
|---|---|
| `dyyl <file>` | 预扫描（填未填的 `ai.auto`）→ 执行脚本 |
| `dyyl build <file>` | 重置所有 `ai.auto.filled` → `ai.auto` → 预扫描（重新填）→ 不执行 |
| `dyyl install/update/remove/...` | 现有插件管理子命令，不变 |

解析逻辑（伪代码）：
```rust
let first = args.get(1);
match first.map(String::as_str) {
    Some("install") | Some("update") | Some("remove") | Some("autoremove") | Some("list")
        => cli_plugin_dispatch(args, lang),       // 现有插件子命令
    Some("build") if args.len() == 3
        => prepass::build_only(&args[2], lang),   // 新增：重置+预扫描，不执行
    _
        => run_script(args, lang),                // 现有 --flag <file> 逻辑（含预扫描）
}
```

`--lang` / `--debug` 等全局选项在子命令前使用：`dyyl --lang zh build script.dyyl`。

### 8.2 `run_script` 改动

现有 `run_script`（[src/main.rs](file:///workspace/src/main.rs)）在 parser 之前插入预扫描：

```rust
fn run_script(args, lang) {
    // ... 解析 --debug --lang <file> ...
    let file = &args[file_arg];

    // 新增：预扫描
    prepass::run(file, lang)?;

    // 现有逻辑：读文件 → 解析 → 执行
    let content = fs::read_to_string(file)?;
    let commands = parser::parse(&content)?;
    runtime::execute(commands, ...);
}
```

### 8.3 `prepass::run` 流程

```rust
pub fn run(file: &Path, lang: Lang) -> Result<(), PrepassError> {
    let content = fs::read_to_string(file)?;

    // 1. 扫描所有 ai.auto（未填的）
    let placeholders = scan_placeholders(&content);

    // 2. 无未填 → 直接返回
    if placeholders.is_empty() {
        return Ok(());
    }

    // 3. 加载凭证（缺失则交互提示）
    let creds = credentials::ensure_ai(lang)?;

    // 4. 构造批量请求
    let provider = ai::build_provider(&creds.ai);
    let (system, user_prompt) = prompt::build_batch(&content, &placeholders);

    // 5. 请求 AI（含重试 3 次）
    let response = provider.ask(&system, &user_prompt)
        .map_err(|e| PrepassError::AiFailed(e))?;

    // 6. 解析响应
    let values = prompt::parse_response(&response)
        .map_err(|e| PrepassError::ParseFailed(e))?;

    // 7. 回写
    let new_content = rewrite_placeholders(&content, &placeholders, &values);
    fs::write(file, new_content)?;

    Ok(())
}
```

### 8.4 `prepass::build_only` 流程

```rust
pub fn build_only(file: &Path, lang: Lang) -> Result<(), PrepassError> {
    let content = fs::read_to_string(file)?;

    // 1. 重置所有 ai.auto.filled → ai.auto
    let reset = reset_filled(&content);
    fs::write(file, reset)?;

    // 2. 调 prepass::run（重新填）
    run(file, lang)
}
```

### 8.5 `reset_filled` 规则

把所有 `ai.auto.filled <提示>, <值>` 替换回 `ai.auto <提示>`：
- `ai.auto.filled "提示", "值"` → `ai.auto "提示"`
- `ai.auto.filled _, "值"` → `ai.auto`
- `ai.auto.filled "提示", 25565` → `ai.auto "提示"`

### 8.6 `scan_placeholders` 规则

逐行扫描，找以下模式（行内任意位置）：

| 模式 | 占位符 |
|---|---|
| `ai.auto` （独立词，后无参数） | 提示为空 |
| `ai.auto "..."` 或 `ai.auto '...'` 或 `ai.auto bareword` | 提示为引号内容或裸词 |

注意区分 `ai.auto` 与 `ai.auto.filled`（后者是已填的，不扫描）。

### 8.7 `rewrite_placeholders` 规则

按占位符 ID 回写：
- 提示为空 + AI 返回 string → `ai.auto.filled _, "value"`
- 提示为空 + AI 返回 number → `ai.auto.filled _, 42`
- 提示非空 + AI 返回 string → `ai.auto.filled "提示", "value"`
- 提示非空 + AI 返回 number → `ai.auto.filled "提示", 42`

字符串值需转义：双引号包裹，转义 `"` `\` 换行等（与现有引号规则一致）。

### 8.8 `--debug` 诊断

- 预扫描阶段：`--debug` 输出扫描到的占位符数量、AI 请求 URL、响应状态、回写的占位符 ID 列表。
- `build` 子命令：输出重置的 `ai.auto.filled` 数量、重新填的占位符数量。

### 8.9 不引入 `--no-prepass` 标志

预扫描是无条件的前置步骤（未填的 `ai.auto` 必须处理才能执行）。若用户想跳过 AI 请求，必须先 `dyyl build` 填好所有占位符，之后 `dyyl <file>` 检测到无未填占位符会跳过预扫描的 AI 请求部分。

---

## 9. 错误处理与哨兵模型

### 9.1 与 dyyl 现有错误哲学对齐

dyyl 现有哲学（决策记录 #3）：**完全不报错不中止**，按返回类型分哨兵：
- 数值类 → `-1`
- 字符串类 → `""`
- 逻辑类 → `0`（假）
- 字典类 → 空字典
- 列表类 → 空列表
- `--debug` 时 stderr 弹警告（命令名 + 行号 + 原因）

### 9.2 各命令的哨兵

| 命令 | 类型 | 成功返回 | 失败返回 | 失败时 --debug |
|---|---|---|---|---|
| `ai.ask` | 字符串类 | AI 响应字符串 | `-1` | stderr: `line N: ai.ask failed: <reason>` |
| `ai.auto.filled` | 取值（不请求 AI） | 提示对应的值 | 不会失败（值是字面量） | — |
| `ai.auto`（未填，预扫描未回写成功） | — | — | 脚本无法执行 | 预扫描中止 |
| `logic.end`（栈空） | 逻辑类 | `1` | `0` | stderr: `line N: logic.end without open block` |

### 9.3 `ai.ask` 失败的 `-1` 与现有字符串哨兵的冲突

`ai.ask` 按类型属字符串类，现有哨兵应为 `""`（空字符串）。但本设计选了 `-1`。

**决策**：`ai.ask` 是**特例**，失败返回 `-1`（数值），而非空字符串。原因：
- 空字符串是合法的 AI 响应（AI 可能返回空内容），无法区分"AI 返回空"与"请求失败"。
- `-1` 作为数值，与字符串类型的成功返回值类型不同，用户可显式判断：
  ```dyyl
  set $a, ai.ask "..."
  logic.if logic.same($a, -1), _
    io.out "AI 请求失败"
  logic.end
  ```
- `--debug` 时 stderr 输出原因，便于诊断。

**注意**：这违反了 dyyl "类型一致哨兵"的惯例，但是合理特例（与 `str.to.num` 解析失败返回 `-1` 一致——也是字符串命令返回数值哨兵）。

### 9.4 预扫描失败：中止脚本（特例）

预扫描失败（AI 请求失败、解析失败、回写失败）会**中止脚本**，这与 dyyl "不中止"哲学冲突。

**理由**：
- 预扫描是**执行前**阶段，不是脚本运行时。此时还没有"命令返回值"的概念。
- 未填的 `ai.auto` 无法求值（它需要 AI 填值才能变成 `ai.auto.filled`），跳过会导致语法错误。
- 中止是唯一合理选择。

**退出码**：
- 预扫描失败 → 退出码 2（区别于脚本运行时错误的退出码 1）。
- stderr 输出：`prepass failed: <reason>`（含 AI 错误详情、重试次数）。

### 9.5 凭证提示失败

交互式提示过程中用户 Ctrl+C 或 EOF（无 stdin）→ 中止，退出码 3。

stderr：`credential input aborted`。

### 9.6 插件凭证预检失败

插件 manifest 声明 `credentials.fields` 但：
- 用户在交互提示时拒绝（Ctrl+C）→ 中止脚本，退出码 3。
- `set_credentials` 返回非 0 → 插件拒绝凭证，dyyl 拒绝加载该插件 → RuntimeError + 哨兵（与现有插件加载失败一致，脚本继续）。

### 9.7 `logic.end` 错误

| 情况 | 行为 |
|---|---|
| `logic.end` 栈空（无开放块） | 哨兵 `0`，`--debug` stderr 警告，该行无操作，继续执行 |
| 文件结束栈非空（开放块未关闭） | **中止脚本**，退出码 4，stderr: `unclosed block at line <start>` |
| 显式块 N 行内嵌套开放块但 `logic.end` 在 N 行外 | 隐式导致"未关闭块"，文件结束时报错 |

文件结束未关闭块是语法错误，无法继续（栈未清空，执行流不一致）。

### 9.8 错误模型总结

| 阶段 | 失败行为 | 退出码 |
|---|---|---|
| 预扫描 | 中止 | 2 |
| 凭证提示 | 中止 | 3 |
| 未关闭块 | 中止 | 4 |
| `ai.ask` 运行时 | 哨兵 `-1`，继续 | 0（脚本正常结束） |
| `logic.end` 栈空 | 哨兵 `0`，继续 | 0 |
| 插件加载（含凭证预检） | RuntimeError + 哨兵，继续 | 0 |
| 现有命令（math/str/...） | 哨兵，继续 | 0（不变） |

### 9.9 `--debug` 输出格式

与现有 [src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs) 的 `warn_block_underdeclared` 风格一致：

```
line N: <command text>
reason: <i18n 消息>
```

新增 i18n 键（加到 [locales/en.json](file:///workspace/locales/en.json) + [locales/zh.json](file:///workspace/locales/zh.json)）：
- `ai.ask_failed`：`"ai.ask failed: {reason}"` / `"ai.ask 失败: {reason}"`
- `ai.prepass_failed`：`"prepass failed: {reason}"` / `"预扫描失败: {reason}"`
- `ai.credential_aborted`：`"credential input aborted"` / `"凭证输入中止"`
- `logic.end_without_open`：`"logic.end without open block"` / `"logic.end 无开放块"`
- `logic.unclosed_block`：`"unclosed block at line {line}"` / `"第 {line} 行块未关闭"`
- `ai.credential_prompt_provider`：`"Provider (1=openai-chat, 2=openai-response, 3=anthropic)"` / 同中文
- `ai.credential_prompt_key`：`"API Key"` / 同
- 等凭证提示相关键

---

## 10. 测试策略

### 10.1 测试分层

| 层 | 目的 | 工具 |
|---|---|---|
| 单元（ai/*） | Provider 请求构造、响应解析、批量 prompt 构造 | `cargo test` + mock HTTP |
| 单元（credentials.rs） | credentials.toml 读写、字段预检、权限警告 | `cargo test` + tempdir |
| 单元（prepass.rs） | 占位符扫描、回写、reset_filled | `cargo test` + tempdir |
| 单元（logic.end） | 开放块栈、嵌套、错误情况 | `cargo test` + fixture 脚本 |
| 集成（ai.ask/ai.auto） | 端到端：脚本调用 → mock AI → 返回值 | `cargo test` + mock HTTP server |
| 集成（插件凭证） | manifest 声明 → 预检 → set_credentials 注入 | `cargo test` + fixture cdylib |
| i18n 双语 | 新增键 en/zh 覆盖、fallback | 现有 `tests/i18n_tests.rs` 扩展 |

### 10.2 mock AI HTTP server

用 `tokio` + `hyper`（或 `wiremock` crate）起本地 HTTP server 模拟三种 Provider 端点：
- `/v1/chat/completions` → 返回固定 Chat Completions 响应
- `/v1/responses` → 返回固定 Responses API 响应
- `/v1/messages` → 返回固定 Anthropic 响应

测试时 credentials.toml 的 `base_url` 指向 `http://localhost:<port>`，绕过真实 API。

### 10.3 关键测试用例

**ai 模块**

- `ai.ask` 单参数 → 用内置 system prompt，请求体含 `system` message，返回响应文本 ✓
- `ai.ask` 双参数 → 用用户 system，请求体含用户 system ✓
- `ai.ask _, prompt` → 用内置 system（与单参数等价）✓
- `ai.ask` 失败（mock 返回 500，重试 3 次后仍失败）→ 返回 `-1` ✓
- `ai.ask` 401 → 不重试，返回 `-1` ✓
- `ai.ask` 429 + `Retry-After: 1` → 重试 3 次，仍 429 → 返回 `-1` ✓
- 三种 Provider 各自的请求体格式正确（headers、body schema）✓
- 三种 Provider 各自的响应解析正确（提取文本字段）✓
- base_url 自定义 → 请求发往自定义端点 ✓
- 超时 1800s（不实际等待，用 mock 延迟 + 较短超时 override 验证超时逻辑）✓

**prepass 模块**

- `scan_placeholders`：识别 `ai.auto`、`ai.auto "提示"`、`ai.auto bareword`，不误识别 `ai.auto.filled` ✓
- `scan_placeholders`：行内任意位置（`set $x, ai.auto "..."`、`file.write ai.auto, ...`）✓
- `build_batch`：构造的 user prompt 含 `<<<AUTO_<id>: <hint>>>` 标记 + 完整文件内容 ✓
- `parse_response`：标准 JSON `{"1":{"type":"string","value":"x"}}` ✓
- `parse_response`：AI 返回 ```` ```json {...} ``` ```` 包裹 → 剥离后解析 ✓
- `parse_response`：AI 返回带前后解释文本 → 提取 `{...}` 子串解析 ✓
- `parse_response`：JSON 缺某 ID → 该占位符保持未填，警告 ✓
- `rewrite_placeholders`：提示为空 + string → `ai.auto.filled _, "value"` ✓
- `rewrite_placeholders`：提示为空 + number → `ai.auto.filled _, 42` ✓
- `rewrite_placeholders`：提示非空 + string → `ai.auto.filled "提示", "value"` ✓
- `rewrite_placeholders`：提示非空 + number → `ai.auto.filled "提示", 42` ✓
- `rewrite_placeholders`：值含特殊字符（`"` `\` 换行）→ 正确转义 ✓
- `reset_filled`：`ai.auto.filled "提示", "值"` → `ai.auto "提示"` ✓
- `reset_filled`：`ai.auto.filled _, "值"` → `ai.auto` ✓
- `reset_filled`：`ai.auto.filled "提示", 42` → `ai.auto "提示"` ✓
- 预扫描无未填占位符 → 跳过 AI 请求 ✓
- 预扫描有未填 → 触发 AI 请求 → 回写 → 落盘 ✓

**credentials 模块**

- `credentials.toml` 不存在 → 首次 `ai.ask` 触发交互提示（mock stdin）→ 写入文件 ✓
- `[ai]` 段缺 `api_key` → 触发提示补齐 ✓
- 插件 manifest 声明 `credentials.fields` 但 `[plugin.x]` 段缺失 → 触发提示 ✓
- 插件 `[plugin.x]` 段存在但缺某字段 → 提示补齐缺失字段 ✓
- `secret: true` 字段 → 输入不回显（复用 io.inpasswd 机制）✓
- `secret: false` 字段 → 正常回显 ✓
- 文件已存在但权限非 0600 → 不修正 + debug 警告 ✓
- 新建文件 → 不强制权限断言 ✓
- stdin EOF（无交互环境）→ 中止，退出码 3 ✓

**logic.end 模块**

- `logic.if cond, _` + `logic.end` → 块体执行 ✓
- `logic.while cond, _` + `logic.end` → 循环执行 ✓
- `logic.for n, _` + `logic.end` → 定次循环 ✓
- `logic.else cond, _` + `logic.end` → elif 语义 ✓
- 嵌套开放块（while + if）→ 栈式关闭 ✓
- 开放块嵌套显式块 → 显式块不进栈 ✓
- 显式块嵌套开放块 → 开放块在显式块 N 行内关闭 ✓
- `logic.end` 栈空 → 哨兵 `0` + debug 警告 ✓
- 文件结束栈非空 → 中止，退出码 4 ✓
- 显式行数与 `_` 混用 → 各自正确 ✓
- 现有显式行数脚本无需修改 → 行为不变 ✓

**插件凭证集成**

- 插件 manifest 无 `credentials` 段 → 跳过预检，正常加载 ✓
- 插件 manifest 有 `credentials.fields`，credentials.toml 全有 → 调 `set_credentials` 注入 JSON ✓
- 插件 manifest 有 `credentials.fields`，缺字段 → 交互提示补齐 → 注入 ✓
- `set_credentials` 返回非 0 → 插件拒绝凭证，RuntimeError + 哨兵，脚本继续 ✓
- ABI v1 插件（无 `set_credentials` 符号）→ 跳过注入，仍能加载（v1 兼容）✓
- ABI v2 插件必须实现 `set_credentials` → 缺符号则加载失败 ✓

**集成（端到端）**

- 脚本含 `ai.auto "端口"` → `dyyl <file>` 预扫描填值 → 执行用填的值 ✓
- 脚本含 `ai.auto`（无提示）→ 预扫描用 `<<<HERE>>>` 标记推断 → 填值 ✓
- 脚本含多个 `ai.auto` → 批量单请求 → 全部填值 ✓
- `dyyl build <file>` → 重置所有 `ai.auto.filled` → 重新填值 → 不执行 ✓
- `dyyl <file>`（已填文件）→ 预扫描无未填 → 跳过 AI 请求 → 直接执行 ✓
- `ai.ask` 运行时请求 → 返回字符串 ✓
- `ai.ask` 失败 → 返回 `-1`，脚本继续 ✓

**i18n 双语**

- 新增键 en/zh 都有（`missing_translations` 返回空）✓
- `--lang zh` 下 `ai.ask` 失败 stderr 输出中文 ✓
- `--lang en` 下输出英文 ✓
- 凭证提示问题在 en/zh 下都正确 ✓

### 10.4 不测的

- 不测真实 OpenAI/Anthropic API（用 mock HTTP server）
- 不测 1800s 真实超时（用 mock + override 超时参数验证逻辑）
- 不测跨平台凭证文件权限（仅 Linux，macOS/Windows 靠文档说明）
- 不测 ABI v1 → v2 真实插件升级（用 fixture cdylib 模拟）

### 10.5 与现有测试套件集成

- `cargo test` 一键跑全部
- 新增 `tests/ai_tests.rs`（ai 模块单元 + 集成）
- 新增 `tests/prepass_tests.rs`
- 新增 `tests/credentials_tests.rs`
- 新增 `tests/logic_end_tests.rs`（或扩展现有 `tests/logic_tests.rs`）
- 新增 `tests/plugin_credentials_tests.rs`（或扩展 `tests/plugin_e2e_tests.rs`）
- 新增 `tests/fixtures/ai-*.dyyl` golden 脚本
- `cargo fmt --check` + `cargo clippy --all-targets --all-features` 必须通过

---

## 11. 实现顺序（高层，详细计划由 writing-plans 产出）

**阶段 1：凭证系统基础**

1. `credentials.rs`：credentials.toml 读写 + 交互式提示
2. `ai/` 模块：AiProvider trait + 三种 Provider 实现 + HTTP 客户端（重试 3 次 + 超时 1800s）

**阶段 2：AI 命令**

3. `runtime/cmd/ai.rs`：`ai.ask` handler（运行时 HTTP）+ `ai.auto.filled` handler（取值）
4. `prepass.rs`：占位符扫描 + 批量请求构造 + 响应解析 + 回写 + `reset_filled`
5. `main.rs`：`build` 子命令分发 + `run_script` 集成预扫描

**阶段 3：logic.end**

6. `exec_block.rs` + `execute.rs`：开放块预扫描 + 栈式关闭 + 嵌套支持

**阶段 4：插件凭证**

7. `manifest.rs`：`credentials` 段解析
8. `abi.rs`：ABI v2，新增 `set_credentials` 符号
9. `loader.rs`：`on_load` 前预检 + 注入

**阶段 5：i18n + 测试**

10. `locales/en.json` + `locales/zh.json`：新增键
11. 测试套件（按第 10 节）
12. 文档（README + dyyl-api-reference.md 增 ai/credentials/logic.end 章节）

---

## 12. 开放问题（实现时再决）

- `ai.ask` 是否需要支持 `temperature` / `max_tokens` 等请求参数？v1 不支持，仅 5 字段最小集；后续若需求出现可加 `[ai].request_params` 段。
- `ai.auto` 的批量请求若占位符过多（如 > 50 个）是否分批？v1 不分批，单次请求；后续可加分批策略。
- 凭证文件是否支持 `keyring`（系统密钥库）？v1 仅明文 TOML；后续可加 `backend = "keyring"` 选项。
- `logic.end` 是否支持 `break` / `continue`？v1 不支持，仅 `logic.end` 关闭块；后续可加 `logic.break` / `logic.continue`。
- ABI v2 的 `set_credentials` 是否需要支持增量更新（插件运行中凭证变更）？v1 仅 `on_load` 前注入一次；后续可加 `refresh_credentials` ABI 函数。
- `dyyl build` 是否支持指定单个占位符刷新（`dyyl build <file> --only <id>`）？v1 全部刷新；后续可加。
