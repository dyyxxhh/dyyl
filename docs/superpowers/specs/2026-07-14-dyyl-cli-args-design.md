# DYYL 脚本命令行参数解析 — 设计文档

**日期**: 2026-07-14
**主题**: 让 DYYL 脚本能够解析文件名之后的命令行参数(如 `a.dyyl --help`)
**状态**: 已通过 brainstorming 设计评审,待 implementation plan

---

## 1. 背景与动机

当前 `dyyl` 解释器([src/main.rs](../../../src/main.rs))只识别自身的 flag(`--debug`/`--host-json`/`--lang <en|zh>`)与一个文件名。文件名之后的参数要么被误判为"未知 flag"导致报错退出,要么在多个非 flag 参数情况下静默覆盖 `filename` 变量。脚本无法读到这些参数。

用户希望:`./a.dyyl --help` 或 `dyyl a.dyyl --help foo bar` 这样调用时,脚本内部能读到 `--help`、`foo`、`bar` 并自行解析。

**Shebang 已可用**:DYYL lexer 把未在引号内的 `#` 视为注释起止([src/lexer/mod.rs](../../../src/lexer/mod.rs) `preprocess_line`),所以脚本首行 `#!/usr/bin/env dyyl` 会被当作注释跳过,无需特殊处理。

## 2. 设计决策

| # | 决策点 | 选择 | 理由 |
|---|---|---|---|
| Q1 | 模式 | 脚本驱动 — 解释器不内置 `--help`,只负责把参数转发给脚本 | 符合 DYYL "小而可预测、命令式" 哲学;解释器保持轻量 |
| Q2 | 参数范围 | 仅文件名之后的参数(`dyyl --debug a.dyyl --help` → 脚本看到 `["--help"]`) | Unix 惯例;避免脚本 flag 与解释器 flag 撞名 |
| Q3 | 命令族 | `cli.*` | 与 `system.*`(os/arch)职责分离,后续可扩展 |
| Q4 | 子命令 | 6 个:`args`/`count`/`get`/`has`/`value`/`script_name` | 覆盖 `--help` 检查、按索引取参、`--key value`/`--key=value` 取值、脚本自身 basename |
| Q5 | `cli.value` 语法 | 同时识别 `--key value`(空格分隔)与 `--key=value`(等号) | 两种 Unix 常见写法都支持 |
| Q6 | `cli.has` 语义 | 精确匹配 + `=value` 后缀算存在;不做前缀匹配 | 与 `cli.value` 行为一致;前缀匹配太 magic |
| Q7 | `cli.get` 越界 | 返回 `Value::Num(-1)` | 与 `list.get` 越界行为一致([src/runtime/cmd/list_handlers.rs](../../../src/runtime/cmd/list_handlers.rs)) |
| Q8 | `cli.script_name` | 返回 basename | 简洁,符合"脚本名"语义 |
| Q9 | `--` 分隔符 | 原样转发,不做特殊处理 | 解释器保持轻量;`--` 只是普通字符串参数 |
| Q10 | 预置变量 | 不预置 `$argv`/`$argc` | 显式优于隐式,符合 DYYL 现有 `create.*` 约定 |
| Q11 | `dyyl --help` | 不在范围内 | YAGNI,本次只做脚本级参数解析 |

## 3. 用户可见的 `cli.*` API

新增 6 个命令,全部属于 `cli.*` 家族。所有命令只读,不修改 Env 状态。

| 命令 | 参数 | 返回值 | 语义 |
|---|---|---|---|
| `cli.args` | 无 | `Value::List<Vec<Value::Str>>` | 文件名之后的所有参数,按原顺序。无参时返回空列表。 |
| `cli.count` | 无 | `Value::Num(n)` | 参数个数。无参时返回 0。 |
| `cli.get <idx>` | 一个非负整数 | `Value::Str(s)` 或 `Value::Num(-1)` | 0-based 下标取参数。越界或负数返回 `Value::Num(-1)`(与 `list.get` 一致)。 |
| `cli.has <flag>` | 一个字符串 | `Value::Num(1)` 或 `Value::Num(0)` | 精确匹配 flag token,或匹配 `--flag=...` 形式(`--flag` 部分相等即算存在)。不做前缀匹配。 |
| `cli.value <flag>` | 一个字符串 | `Value::Str(s)` 或 `Value::Empty` | 同时识别 `--flag value`(空格分隔)与 `--flag=value`(等号)。找不到 flag 或 flag 后无值(下一个 token 以 `-` 开头,或已是最后一个)→ 返回 `Value::Empty`。同一 flag 多次出现 → 返回**第一个**值。 |
| `cli.script_name` | 无 | `Value::Str(s)` | 脚本文件名的 basename(用 `Path::file_name` 取)。例:`/home/user/a.dyyl` → `a.dyyl`。 |

### 示例脚本 `a.dyyl`

```dyyl
#!/usr/bin/env dyyl
logic.if cli.has("--help"), _
  io.out "Usage: a.dyyl [--help] [--out FILE] [args...]"
  io.out "  --help       show this help"
  io.out "  --out FILE   output to FILE"
  logic.end
logic.if cli.has("--out"), _
  io.out str.join("output goes to: ", cli.value("--out"))
  logic.end
```

> 注:上面假设 `logic.if` 把 `Value::Num(0)` 视为假、非 0 视为真,`cli.has` 直接返回 1/0 即可作为条件。`str.join` 是否存在以及能否拼接 `Value::Str` 由实现阶段核实;若不存在则改用 `io.out` 拼接或新增辅助命令。

调用示例:
- `./a.dyyl --help` → 打印用法
- `dyyl a.dyyl --out result.txt foo bar` → 打印 `output goes to: result.txt`

## 4. 架构与组件

### 4.1 实现方案:Env 新增字段 + 新建 `src/runtime/cmd/cli.rs`

(对比了"ExecContext 携带 args" 与 "thread_local 全局" 两种方案,选定此方案。理由:与现有 `Env` 持有 `host_provider`/`plugin_manager` 等状态的模式一致;Rust 惯例;易测试;无全局状态陷阱。)

### 4.2 [src/main.rs](../../../src/main.rs) 参数解析改动

当前 main.rs 的循环(L37-L68)把所有 args 一视同仁地处理,导致文件名之后的 `--xxx` 会被误当成解释器未知 flag 报错,或多个非 flag 参数静默覆盖 filename。

**新逻辑**:一旦识别到 filename,就停止解析解释器 flag,把剩余所有 args(包括 `--xxx`、`-x`、`--`、普通字符串)原样收集为脚本参数。

```rust
let mut i = 1;
let mut script_args: Vec<String> = Vec::new();
while i < args.len() {
    if !script_args.is_empty() || filename.is_some() {
        // 已经看到 filename,后续全部收集为脚本参数
        script_args.push(args[i].clone());
        i += 1;
        continue;
    }
    match args[i].as_str() {
        "--debug" => debug = true,
        "--host-json" => host_json = true,
        "--lang" => { /* 现有逻辑不变 */ },
        other if !other.starts_with('-') => filename = Some(other.to_string()),
        _ => { /* 现有 unknown option 错误 */ }
    }
    i += 1;
}
```

**副作用修复**:目前 `dyyl a.dyyl b.dyyl` 会把 filename 静默改成 `b.dyyl`;新逻辑下 `b.dyyl` 会变成 `a.dyyl` 的脚本参数(更可预测)。

### 4.3 [src/runtime/env.rs](../../../src/runtime/env.rs) 改动

`Env` 新增两个字段 + setter/getter:

```rust
pub struct Env {
    bindings: HashMap<String, Value>,
    lang: Cell<Lang>,
    host_provider: Option<Arc<dyn HostProvider>>,
    game_scope: GameChooseScope,
    mcm_id_counter: Cell<u64>,
    plugin_manager: Arc<PluginManager>,
    script_args: Vec<String>,   // 新增,默认空 Vec
    script_name: String,        // 新增,默认空 String
}

impl Env {
    pub fn set_script_args(&mut self, args: Vec<String>) { self.script_args = args; }
    pub fn script_args(&self) -> &[String] { &self.script_args }
    pub fn set_script_name(&mut self, name: String) { self.script_name = name; }
    pub fn script_name(&self) -> &str { &self.script_name }
}
```

`Env::new()` 初始化为空 `Vec`/空 `String`,所以**不调用 `cli.*` 的脚本完全不受影响**。

**`script_name` 字段存的是原始 filename 字符串**(main.rs 把 `filename` 变量原样传入),basename 提取由 `cli.script_name` handler 用 `Path::new(env.script_name()).file_name()` 完成。这样 `Env` 只持有原始数据,转换在边缘(handler)进行,便于测试不同路径形式。

### 4.4 [src/runtime/execute.rs](../../../src/runtime/execute.rs) 改动

新增 `run_script_with_lang_and_args` 函数(保留原 `run_script_with_lang` 不变,避免破坏现有测试):

```rust
pub fn run_script_with_lang_and_args(
    source: &str,
    debug: bool,
    lang: Lang,
    args: Vec<String>,
    script_name: String,
) -> ScriptOutput {
    // ... parse source ...
    let mut env = Env::new();
    env.set_lang(lang);
    env.set_script_args(args);
    env.set_script_name(script_name);
    // ... exec ...
}
```

同样为 `run_script_with_lang_and_host` 增加一个 `_and_args` 变体(用于 `--host-json` 模式),签名中加入 `args: Vec<String>` 与 `script_name: String`。

main.rs 根据是否 `host_json` 选择调用对应的 `_and_args` 变体。

### 4.5 新建 [src/runtime/cmd/cli.rs](../../../src/runtime/cmd/cli.rs)

路由入口 `handle_cli_command(call, env, ctx)`,内部 match `&call.command["cli.".len()..]` 分发到 6 个 handler。每个 handler 用 `env.script_args()` 读取参数,用 `env.script_name()` 读取脚本名。

handler 函数命名约定(与现有 `system.rs` 一致):
- `handle_cli_args` / `handle_cli_count` / `handle_cli_get` / `handle_cli_has` / `handle_cli_value` / `handle_cli_script_name`

### 4.6 [src/runtime/cmd/dispatch.rs](../../../src/runtime/cmd/dispatch.rs) 注册

在 L60-L62 附近(system.* 之后)加:

```rust
cmd if cmd.starts_with("cli.") => cli::handle_cli_command(call, env, ctx),
```

并在 [src/runtime/cmd/mod.rs](../../../src/runtime/cmd/mod.rs) 加 `pub(crate) mod cli;`。

## 5. 数据流

```
shell: $ ./a.dyyl --help foo
  ↓
kernel exec: /usr/bin/env dyyl ./a.dyyl --help foo
  ↓
main.rs argv = ["dyyl", "./a.dyyl", "--help", "foo"]
  - i=1: "./a.dyyl" → filename = "./a.dyyl"
  - i=2: "--help" → filename 已设,script_args.push("--help")
  - i=3: "foo"    → script_args.push("foo")
  ↓
main.rs 调用 run_script_with_lang_and_args(source, debug, lang, ["--help","foo"], "./a.dyyl")
  ↓
execute.rs: env.set_script_args(["--help","foo"]); env.set_script_name("./a.dyyl")
  ↓
脚本运行 cli.has("--help") → handler 读 env.script_args() → 返回 Num(1)
脚本运行 cli.script_name   → handler 读 env.script_name() → 返回 Str("a.dyyl")
                                  ↑ Path::new("./a.dyyl").file_name() = "a.dyyl"
```

## 6. 错误处理

| 场景 | 行为 |
|---|---|
| `cli.get` 越界(下标 >= count) | 返回 `Value::Num(-1)`,debug 模式下 stderr 警告(与 `list.get` 一致) |
| `cli.get` 负数下标 | 返回 `Value::Num(-1)`(不支持负下标) |
| `cli.get` 参数非整数 | 返回 `RuntimeError`(i18n: expected integer) |
| `cli.has`/`cli.value` 参数非字符串 | 按现有 `eval_expr` 行为,数字会被转成字符串再匹配 |
| `cli.value` 找不到 flag | 返回 `Value::Empty`(不报错) |
| `cli.value` flag 后无值 | 返回 `Value::Empty`(下一个 token 以 `-` 开头,或已是最后一个) |
| `cli.args`/`cli.count`/`cli.script_name` 多传参数 | 返回 `RuntimeError`(i18n: 不接受参数) |
| 调用未知 `cli.xxx` | 返回 `RuntimeError`(i18n: unknown command,与 `system.*` 未知子命令一致) |

## 7. 测试

新建 `tests/cli_args_tests.rs`,涵盖以下用例:

1. `cli.args` 基础:`vec!["--help", "foo"]` → 列表含两个元素
2. `cli.args` 空列表:无参时返回空列表
3. `cli.count`:`vec!["a","b","c"]` → 3
4. `cli.get` 正常:`cli.get 0` → 第一个参数;`cli.get 2` → 第三个
5. `cli.get` 越界:`cli.get 10` → `-1`
6. `cli.get` 负数:`cli.get -1` → `-1`
7. `cli.has` 精确匹配:`["--help"]` → `cli.has "--help"` 返回 1
8. `cli.has` 等号后缀:`["--mode=fast"]` → `cli.has "--mode"` 返回 1;`cli.has "--mode=fast"` 也返回 1
9. `cli.has` 不前缀匹配:`["--help"]` → `cli.has "--h"` 返回 0;`["--helper"]` → `cli.has "--help"` 返回 0
10. `cli.value` 空格分隔:`["--out", "foo.txt"]` → `cli.value "--out"` 返回 `foo.txt`
11. `cli.value` 等号:`["--mode=fast"]` → `cli.value "--mode"` 返回 `fast`
12. `cli.value` 找不到:`["--help"]` → `cli.value "--out"` 返回 `empty`
13. `cli.value` flag 后无值:`["--out"]` → 返回 `empty`;`["--out", "--verbose"]` → 返回 `empty`
14. `cli.value` 多次出现:`["--out", "a", "--out", "b"]` → 返回 `a`(first wins)
15. `cli.script_name` basename:`script_name="/home/user/a.dyyl"` → 返回 `a.dyyl`
16. `--` 原样转发:`["--help", "--", "--foo"]` → `cli.args` 返回三元素列表(含 `--`)
17. 向后兼容:不调用 `cli.*` 的脚本完全不受影响(无参情况下 `Env` 字段为空)
18. fixture 脚本:新建 `tests/fixtures/cli-args.dyyl`,演示 6 个命令的典型用法,作为 golden test

测试通过 `run_script_with_lang_and_args` 注入参数,不依赖进程级 argv。

## 8. 文档

更新 [README.md](../../../README.md):
- "CLI usage" 段加一句:文件名之后的参数会原样传给脚本,可通过 `cli.*` 命令读取
- 新增 "Script command-line arguments" 小节,列出 6 个命令的语义表(同第 3 节),含 shebang 直接调用示例
- "Language basics" 段提一句 `#` 注释天然支持 shebang(`#!/usr/bin/env dyyl`)

更新 [dyyl-api-reference.md](../../../dyyl-api-reference.md):
- 新增 "CLI commands" 章节,与 README 的语义表保持一致,补完整返回值与边界条件

## 9. 范围外(YAGNI)

以下功能本次**不实现**,留待将来按需添加:
- `dyyl --help` / `dyyl -h`(解释器自身帮助)
- `cli.value` 对短 flag cluster 的支持(如 `-abc` 等价 `-a -b -c`)
- `cli.value` 多次出现返回最后一个(`--out a --out b` 返回 `b`)— 当前是 first wins
- `cli.get` 负数下标从末尾取(Python 风格)
- 自动预置 `$argv` / `$argc` 变量
- `cli.args` 排序/去重等高级操作(脚本可用 `list.*` 派生)

## 10. 向后兼容性

- 不调用 `cli.*` 的现有脚本行为完全不变(`Env::new()` 默认空 args/空 script_name)
- 现有测试调用 `run_script_with_lang(source, debug, lang)` 不受影响(保留原签名)
- 现有 `run_script_with_lang_and_host` 签名不变(新增 `_and_args` 变体而非修改)
- main.rs 的"多个非 flag 参数静默覆盖 filename"行为变化:这是一个潜在 bug 修复,理论上无脚本依赖此行为(因为它是静默的)
