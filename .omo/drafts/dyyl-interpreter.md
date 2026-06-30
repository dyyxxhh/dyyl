# dyyl-interpreter 规划草稿

## 状态
- status: awaiting-approval
- pending action: write .omo/plans/dyyl-interpreter.md
- slug: dyyl-interpreter
- 路由: CLEAR + OVERRIDE(用户明确要求被询问,adopt-default filter 关闭)

## 请求
用任意语言制作 dyyl 脚本语言解释器。MCM 部分完全不管(不留占位符)。运行方式 `dyyl filename`。暂时不需要编译(写出完整可编译源码即可)。

## 探查发现(带路径)
- `/x/dyyl/dyyl-api-reference.md`(325 行):唯一现有工件,完整定义 dyyl 命令集。
- `/x/dyyl/`:无任何实现,无 .omo 结构(本次创建 drafts/plans)。
- `/x/mcm/`:Rust 写的 Minecraft mod/游戏管理器(Cargo.toml 依赖 anyhow/clap/reqwest/rusqlite 等)。dyyl 的 `mcm.*` 命令族本用于驱动 mcm,用户明确剔除整个 mcm 部分。
- dyyl 语法:命令式,每行一条命令 `module.subcommand args`;参数逗号分隔;`_`/`empty` 占位;`()` 定界嵌套;`$var` 取值。
- 命令族(文档):create/logic/math/str/dict/io/net/file/user/system/time + mcm(剔除)。
- mcm 的 `Command::Do`(`/x/mcm/src/cli.rs:80`)执行 .mcm 文件,与 dyyl 是不同机制。

## 用户决策记录(6 轮提问,已全部确认)
1. 实现语言 = **Rust**(纯单二进制,与 mcm 同栈)
2. CAS 范围 = **完整 CAS**(sin(π/6)=1/2 等自动化简);落地 = **先查找开源项目,没有再写**
3. 错误处理 = **完全不报错不中止**,按返回类型分哨兵:
   - 数值类命令 → `-1`
   - 字符串类命令 → `""`
   - 逻辑类命令 → `false`
   - 字典类命令 → 空字典
   - 列表类命令 → 空列表
   - debug 模式:任何哨兵返回在 stderr 弹警告(命令名 + 行号 + 原因)
4. 控制流"行数" = **块体行数**(`logic.if/while/for` 行之后的 N 行为块体;文档示例数字视为笔误)
5. 运行方式 = `dyyl <filename>`,**任意扩展名**,按内容解析
6. MCM 部分 = **完全剔除,不留占位符**
7. 赋值/可变性 = **需显式赋值语法**,新增 `set` 命令:`set $var, <右值表达式>`。文档旧示例(math.add 裸名自增)**不兼容,以新语法为准**。
8. 参数切分规则(贪心右值 + 左歧义消歧):
   - 命令有固定参数配额。外层命令从左到右吃参数,吃满配额的逗号后,**剩余部分全部归最后一个参数**(右值贪心读到行尾)。
   - 右值若以命令名开头,则从该命令名读到行尾都是该内层命令的调用,中间逗号属内层,不回外层。
     - 例:`set $i, math.add $i, 1` = set(左值=$i, 右值=math.add($i,1))
   - **歧义只在左侧**:当外层命令的最左参数本身是命令,且两命令逗号总数导致归属不清时,才用占位符 `_`/`empty` 或 `()` 消歧。
9. 受 mcm 影响的非 mcm 命令 = **改用全局绝对路径**,根目录即系统根目录 `/`。net.download 的"路径"参数按绝对路径解释;file.write/file.read 的"文件地址"按绝对路径;user.config 指向 dyyl 自己的配置(或 no-op,实现时定)。
10. 列表数据类型 = **补 `list.*` 命令族**(list.get/list.len/list.append/list.join 等基础操作),让 str.split/dict.keys 返回的列表可索引可遍历。具体命令清单在计划 todo 中按需补全。
11. 可变性模型(两套并存,类 Python):
    - **基本类型(num/str)**:不可变,修改必须走 `set $var, <右值>` 重绑定。
    - **容器(dict/list)**:引用类型,可变。`dict.set`/`list.append` 等原地修改容器内容,不走 set(用户原话:"dict.set 不就是给字典专门用的 set 吗,另一种 set 而已")。
    - 文档 dict 示例 `dict.set freq,...` 后 `dict.get(freq,...)` 取到值,成立。
12. create 命令 = **只接受变量名单参**。num 初值固定 0,str 初值固定 empty。文档 `create.num i, 0` 的 0 视为笔误,解释器只取变量名。要赋初值用后续 `set`。
13. 正则引擎 = **混合策略**:默认 `regex` crate(线性时间、安全);检测到高级语法(前瞻 `(?=)`/后顾 `(?<=)`/回溯/反向引用)时自动切 `fancy-regex`。str.match/extract/replace.regex 统一走这个混合分发器。

## 用户决策记录 - 第 3 轮(9 轮提问,已全部确认)
14. logic.else 链接语义 = **仅链接上一个 if**(不是 else/if 链),即"上一个 if 条件为假 且 本条件为真"时执行。用户原话:"是上一个 if 为假 本条件为真 并不是上一个 else 或者 if 为假"。
15. 控制流嵌套 = **可嵌套**。内层块行数+内层块自身1行 计入外层 N。用户原话:"外层 while 套内层 while 9 行,外层 N = 自身 + 9 + 1;若 N 不足则报错"。块体行数计数是静态可校验的。
16. user.config = **删除**(从文档移除,不实现)。
17. str.rfind 未找到 = **返回 -1**(与 str.find 一致)。
18. user.bash = **保留**(执行 shell 命令,返回输出字符串)。
19. io.get 返回格式 = **按键名字符串**(如 "a"、"Enter"、"Escape"、"Up")。
20. str.format 占位符 = **{N} 索引占位**(`{0}` `{1}` 按序填入)。
21. time.format 格式串 = **YYYY-MM-DD 自定义风格**(`YYYY`年 `MM`月 `DD`日 `HH`时 `mm`分 `ss`秒),非 strftime。
22. math.hash + str.hash = **合并为 math.hash 一个命令**(值可为数值或字符串,自动判断;算法 md5/sha1/sha256)。str.hash 已从文档删除。
23. str.to.num 解析失败 = **返回 -1**(类型哨兵)。

## 用户决策记录 - 第 4 轮(最终一次性扫尾,已全部确认)
24. 数值字面量 = **支持分数/根式/π 源码字面量**(`1/3`、`√2`、`π` 等可直接出现在右值表达式)。
25. list.create = **补充**。列表可通过 `list.create 变量名` 创建空列表,再用 `list.append` 原地追加。
26. 布尔表示 = **数值 1/0**。逻辑命令返回 `1` 真、`0` 假;条件中非 0 为真,0 为假。
27. 变量作用域 = **全局作用域**。块体内 `create.*` 的变量在块外仍可访问。
28. dict.get/list.get 缺失/越界 = **统一返回 -1**。
29. file.write/file.append = **分两命令**。`file.write` 覆盖写入,`file.append` 追加写入。
30. user.bash 返回值 = **成功返回 stdout 字符串,失败返回 -1**。
31. str.escape/unescape = **正则转义/反转义**。
32. str.from.num = **使用 dyyl 规范显示格式**(带分数/根式/π),与 `io.out` 数值输出一致。
33. math.approx = **默认 15 位有效数字**。
34. 命令续行 = **不支持**。每条命令必须在一行内完成。
35. time.weekday = **1=Monday, 7=Sunday**。
36. 注释语法 = **`#` 行注释,支持行首和行内注释**。
37. net.download 返回值 = **成功返回下载字节数,失败返回 -1**。
38. math.strike/math.surplus 负数规则 = **Rust 风格**:整除向 0 取整,余数符号跟 x。
39. math.pow 指数范围 = **支持任意指数**(负数/分数指数返回符号表达式,如 `2^(1/2)=√2`, `2^(-1)=1/2`)。
40. math.round = **数学四舍五入**(.5 远离 0)。
41. str.replace/str.replace.all = **replace 只替换第一个匹配,replace.all 替换所有**。
42. str.split 连续分隔符 = **保留空元素**(`"a,,b"` split `","` -> `["a", "", "b"]`)。
43. list.sort = **按类型升序**。全数值按数值升序,全字符串按字典序升序,混合时数值排在字符串前。
44. io.out 数值输出 = **dyyl 规范显示格式**(⅓、1⅔、√2、π)。
45. time.now = **`YYYY-MM-DD HH:mm:ss`**。
46. logic.same 跨类型 = **类型不同直接不等**(数值 `1` 与字符串 `"1"` 不等)。
47. math.add 混合类型 = **返回 -1**(数值/字符串混合不自动转换)。
48. net.get = **成功返回响应体字符串,失败返回空字符串**。
49. dict 键类型 = **任意类型**(数值/字符串/其他值都可作为键)。
50. io.out 参数 = **单参数**。
51. math.add 数值/字符串混合修正 = **单字符字符串 + 整数走字符码偏移**。`math.add "a", 1` 与 `math.add 1, "a"` 都返回 `"b"`;多字符字符串混合返回 -1;非整数数值(1/2、π、√2)混合返回 -1。
52. math.sub 字符串/整数 = **支持反向字符码偏移**。`math.sub "b", 1` 返回 `"a"`。
53. 字符码规则 = **Unicode 标量值**。非 ASCII 字符也按 Unicode 码点偏移;偏移后不是合法字符时返回空字符串哨兵并在 debug 模式警告。
54. 可选引号 + 反斜杠转义 = **字符串参数默认裸词,无需引号;引号可选。** 参数内含逗号可用 `"..."` 包裹或 `\` 转义(`hello\,world`)。引号内部支持标准转义 `\n` `\t` `\\` `\"`。只有裸露、未转义且不在引号内的逗号才作为参数分隔符。

## 文档修正记录(已全部委托执行完成)
三轮共修正文档错误/缺口:
- 第1轮:math.bash→math.hash、语法示例改 dyyl 原生、变量表补 set、dict 示例修正(create 单参/set/list.len/get/logic.un)、str.index→返回-1、net/file 改绝对路径、新增 list.* 章节(11命令)
- 第2轮:str.rfind 补返回-1、math.hash+str.hash 合并、删 user.config、io.get 补返回格式、str.to.num 补失败返回、str.format 补占位符说明、time.format 补格式说明、logic.else 明确 elif 语义
- 第3轮:补注释/无续行/数值字面量、逻辑返回/嵌套/全局作用域、math/str/dict/list/net/file/io/user/time 的所有最终语义说明,新增 list.create/file.append
- 第4轮:修正 math.add/math.sub 数值+字符串混合语义为单字符 Unicode 码点偏移

## 开源 CAS 调研结论(2 轮 web 调研)
- **`mathcore`**(MIT, crates.io v0.3.1, 51.7KB, 基于 num-bigint/num-rational/nom):精确有理算术(BigRational,1/3+1/6=1/2)、符号表达式树、化简规则、π/e/τ 常量、sqrt、三角函数、方程求解。**最匹配文档需求**。
- 备选:`symbolica`(NOASSERTION 许可,高性能但许可复杂,排除)、`scirs2-symbolic`、`scivex-sym`、`thales`、`arael-sym`。
- **决策**:主选 `mathcore` 作为 CAS 后端。mathcore 不提供文档要求的展示规范(带分数 1⅔、根式括号 (√2)/2、√(3+1)),由 **dyyl 自写 display 格式化层**补齐。
- **fallback 风险**:若 mathcore 在核心化简(1/3×3=1 自动约分、有理+符号混合 1/3+π)不足,回退到基于 num-rational + 自定义 Expr 的轻量 CAS。计划首个 todo 验证 mathcore API 覆盖度。

## Components ledger(6 个独立成败组件)
| id | outcome | evidence path |
|----|---------|---------------|
| C1 CLI+解析器 | `dyyl <file> [--debug]` 读文件,词法/语法分析到命令序列;实现贪心右值+左歧义消歧的参数切分 | dyyl-api-reference.md 语法约定节 + 决策8 |
| C2 值模型+CAS | 数值(mathcore 精确有理+符号)、字符串、字典、列表的表示与运算;set 命令赋值 | create.num 节、math 节 + 决策7 |
| C3 display 格式化 | 数值内部表示→文档规范字符串(带分数>假分数>无理数>小数;根式括号规则) | create.num 输出优先级节 |
| C4 执行引擎+控制流 | 逐行执行、环境/变量、set 赋值、if/while/for 块体行数跳转 | logic 节 + 决策4,7 |
| C5 命令分发+handler | create/set/logic/math/str/dict/list/io/net/file/user/system/time 各模块命令(无 mcm);net/file 用绝对路径 | 各模块节 + 决策6,9,10 |
| C6 错误哨兵+debug | 类型分哨兵返回;--debug 模式 stderr 警告 | 决策3 |

## Open-assumptions(adopted defaults — 均可逆,声明即可不问)
- debug 模式触发:`--debug` CLI 标志(可逆,命名可调)
- 输出优先级:带分数 > 假分数 > 无理数表达式 > 小数近似值;仅在 math.approx 时降级小数(按文档)
- math.add 两字符串拼接、否则数值加(按文档)
- 变量取值 `$var`;create.str/create.num 声明(按文档)
- 暂不编译:实现产出完整可编译 Rust 源码,用 `cargo check` 验证编译;不产出/分发二进制

## 方案
Rust 单 crate 解释器,分层:CLI → 解析器 → 执行引擎(环境+控制流)→ 命令分发 → 各 handler → 值模型(mathcore CAS)→ display 层。错误模型在执行器层把 handler 的 Err 转成类型哨兵,debug 模式弹警告。MCM 命令族完全剔除。

## Gate
所有 fork 已通过完整扫尾提问确认,无遗留 owner-decision。待用户批准后:
1. 跑 `node <skill-root>/scripts/scaffold-plan.mjs dyyl-interpreter --clear`
2. Metis gap 分析
3. APPEND todos 到 .omo/plans/dyyl-interpreter.md
4. 填 TL;DR,自检
5. 呈现摘要 + 问"立即执行 or 先跑高精度 Momus 审查"
