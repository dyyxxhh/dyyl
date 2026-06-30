# dyyl-language-interpreter - Work Plan

## TL;DR (For humans)

**What you'll get:** A Rust implementation of the dyyl scripting language that runs scripts with `dyyl <filename>` and implements the non-mcm parts of the API reference: parsing, variables, math/CAS, strings, dictionaries, lists, files, network, IO, system, and time.

**Why this approach:** The biggest risks are parser ambiguity and symbolic math. The plan locks parser behavior with tests first, then verifies whether `mathcore` can satisfy the CAS requirements before building the rest on top.

**What it will NOT do:** It will not implement `mcm.*` commands, will not package or distribute a release binary, and will not add REPL/editor tooling.

**Effort:** Large
**Risk:** High - custom language parsing + CAS/display semantics + many edge-case commands.
**Decisions to sanity-check:** mcm docs remain but no mcm implementation; unsupported `mcm.*` executes as unknown command sentinel; `mathcore` is only accepted after a spike proves coverage, otherwise use a custom CAS fallback.

Your next move: approve execution with `$start-work`, or request a high-accuracy review first.

---

> TL;DR (machine): Large/high-risk Rust interpreter plan: parser-first TDD, CAS spike/fallback gate, then value model, commands, CLI, fixtures, and final audits.

## Scope

### Must have
- Rust cargo project in `/x/dyyl` with a binary executable named `dyyl` and runnable via `cargo run -- <filename>` during development.
- Script execution model for `dyyl <filename>` with arbitrary filename extension.
- Parser for the current `dyyl-api-reference.md` syntax: comma-separated parameters, `_`/`empty`, parentheses for disambiguation, greedy RHS, left ambiguity disambiguation, inline `#` comments, no continuation lines, direct numeric literals including `1/3`, `√2`, and `π`.
- Value model for num/string/list/dict, global scope, `create.*`, `set`, `$var`, immutable num/string rebinding and mutable list/dict containers.
- CAS/numeric layer meeting documented exact arithmetic and display rules, including mixed fractions, roots, constants, char-code arithmetic, `math.approx` 15 significant digits, and documented math edge cases.
- Implement non-mcm command families from the updated API reference: `logic.*`, `math.*`, `str.*`, `dict.*`, `list.*`, `net.*`, `file.*`, `io.*`, `user.*`, `system.*`, `time.*`.
- Error model: no hard runtime crash for script errors; return command-specific sentinel values and, in `--debug`, warn to stderr with line number, command, and reason.
- Automated tests and golden fixtures for parser, control flow, CAS/display, containers, string/list/dict boundaries, file/net/io/system/time, and debug warnings.

### Must NOT have (guardrails, anti-slop, scope boundaries)
- Do not implement `mcm.*`; do not add mcm handler modules or stubs. Unknown `mcm.*` must use the normal unknown-command sentinel path.
- Do not modify or integrate `/x/mcm`.
- Do not create release packaging, installers, shell completions, REPL, language server, formatter, or syntax highlighter.
- Do not silently reinterpret relative paths as absolute paths; file/net path commands requiring absolute paths must reject relative paths with sentinels.
- Do not rely on public internet or human keyboard input in automated verification.
- Do not broaden “complete CAS” into a general Mathematica/SymPy clone; cover dyyl spec behavior and representative special values.

## Verification strategy

> Zero human intervention - all verification is agent-executed.

- Test decision: TDD with Rust unit/integration tests plus golden fixture scripts.
- Framework: `cargo test`; CLI/golden tests use `assert_cmd` + `predicates` or equivalent dev dependencies.
- Evidence paths:
  - `.omo/evidence/task-<N>-dyyl-language-interpreter.txt` for command output receipts.
  - `.omo/evidence/task-<N>-dyyl-language-interpreter.stderr` for debug-warning receipts.
  - `.omo/evidence/final-dyyl-language-interpreter.txt` for final full-suite + CLI receipts.
- Required commands at final verification:
  - `cargo fmt --check`
  - `cargo test`
  - `cargo run -- tests/fixtures/arithmetic.dyyl`
  - `cargo run -- --debug tests/fixtures/errors-debug.dyyl`
  - local HTTPS fixture test for `net.get` and `net.download` using a generated test certificate trusted only by the test harness.

## Execution strategy

### Parallel execution waves

- **Wave 0 - Spec and scaffold:** confirm current docs, initialize Cargo project, write parser/CAS spike tests.
- **Wave 1 - Parser + value foundation:** parser, AST, value model, error model, environment.
- **Wave 2 - CAS and core commands:** math/display/logic/control flow.
- **Wave 3 - Data/string/files/network/io/time:** string/list/dict/file/net/io/user/system/time handlers.
- **Wave 4 - CLI/golden fixtures/hardening:** end-to-end scripts, debug mode, unknown mcm behavior, local HTTPS networking fixture, final QA.

### Dependency matrix

| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 1 | none | 2,3,4 | none |
| 2 | 1 | 5,6,7 | 3 |
| 3 | 1 | 6 | 2 |
| 4 | 1 | 5-12 | 2,3 |
| 5 | 2,4 | 6,7,8 | 3 |
| 6 | 2,3,4 | 7,8 | none |
| 7 | 5,6 | 8,13 | none |
| 8 | 5,6 | 9-12 | none |
| 9 | 8 | 13 | 10,11,12 |
| 10 | 8 | 13 | 9,11,12 |
| 11 | 8 | 13 | 9,10,12 |
| 12 | 8 | 13 | 9,10,11 |
| 13 | 7-12 | 14 | none |
| 14 | 13 | Final | none |

## Todos

- [x] 1. Initialize Rust project scaffold and dependency spike
  What to do / Must NOT do: Create a Cargo binary crate in `/x/dyyl` named `dyyl`. Add minimal dev-test dependencies. Add a spike test or small internal module that verifies whether `mathcore` supports exact rationals, constants, sqrt, trig special values, parsing, approximation, and expression inspection. If `mathcore` fails any must-have, document fallback to custom CAS in code comments and tests before proceeding. Must not add `/x/mcm` dependency.
  Parallelization: Wave 0 | Blocked by: none | Blocks: 2,3,4
  References: `dyyl-api-reference.md:1-56`; `.omo/drafts/dyyl-interpreter.md` CAS research; Metis findings on CAS gate.
  Acceptance criteria: `cargo test cas_backend_spike` passes and records whether `mathcore` is accepted; `cargo metadata` shows no dependency on `/x/mcm`; if `mathcore` fails `cas_backend_supports_symbolic_sqrt_or_trig_special_values`, the test output must record `CAS_BACKEND=fallback-custom` and subsequent todos must implement the custom CAS path.
  QA scenarios: happy: `cargo test cas_backend_spike -- --nocapture` -> evidence `.omo/evidence/task-1-dyyl-language-interpreter.txt`; failure: `cargo test cas_backend_supports_required_dyyl_cases -- --nocapture` must either pass with mathcore or pass by selecting `CAS_BACKEND=fallback-custom`, never by ignoring the failed case.
  Commit: N | feat(scaffold): initialize dyyl Rust crate and CAS gate

- [x] 2. Build parser lexer with comments, optional-quote strings, backslash escaping, literals, and no-continuation rules
  What to do / Must NOT do: Implement lexical scanning for one command per line, inline `#` comments outside strings, **optional double-quoted strings** (bare words and `"..."` both accepted as string parameters), **backslash `\` escaping** for parameter special characters (`hello\, world` keeps comma in parameter), standard escapes `\n` `\t` `\\` `\"` inside quotes, bare commas only act as parameter delimiters when outside quotes and not backslash-escaped, direct numeric literals (`1/3`, `√2`, `π`), no continuation lines. Must not parse mcm specially.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 5,6,7
  References: `dyyl-api-reference.md:7-30`, `dyyl-api-reference.md:40-56`。
  Acceptance criteria: parser unit tests cover inline comments, bare-word parameters, quoted parameters, backslash-escaped comma, backslash-escaped space, `\n`/`\t` inside quotes, string `#`, bare/unescaped comma not in quotes acts as delimiter, numeric literals, and rejected continuation. Concrete failure fixture `tests/fixtures/parser-continuation-error.dyyl` contains `io.out "a" \` followed by `io.out "b"`; `cargo run -- --debug tests/fixtures/parser-continuation-error.dyyl` must emit sentinel output and stderr containing `line 1`, `continuation`, and the command text. An additional fixture `tests/fixtures/parser-quoting-and-escape.dyyl` (to be created as part of this task) must cover all quoting/escaping combinations.
  QA scenarios: happy: `cargo test parser_lexes_comments_strings_literals_quoting_escaping`; failure: run `cargo run -- --debug tests/fixtures/parser-continuation-error.dyyl`, evidence `.omo/evidence/task-2-dyyl-language-interpreter.txt` and `.stderr`.
  Commit: N | feat(parser): add dyyl lexer with optional quoting and backslash escaping

- [x] 3. Implement command grammar, greedy RHS, parentheses, and placeholder disambiguation
  What to do / Must NOT do: Parse command calls with fixed arity, greedy final argument, nested parenthesized calls, `_`/`empty` placeholders for left ambiguity, and both `math.add $i, 1` and `math.add($i, 1)` forms. Commas inside quoted strings or backslash-escaped (`\,`) are NOT parameter delimiters; only bare, unescaped, unquoted commas split parameters. Do not accept multi-line commands.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 6
  References: `dyyl-api-reference.md:9-30`, `.omo/drafts/dyyl-interpreter.md` decisions 8,34,36 and quoting/escaping decision.
  Acceptance criteria: tests prove `set $i, math.add $i, 1` equals `set $i, math.add($i, 1)` and left-ambiguous call uses `_`/`()`. Quoted strings and backslash-escaped commas do not split parameters. Concrete failure fixture `tests/fixtures/parser-left-ambiguity-error.dyyl` contains an un-disambiguated left-nested call with two comma owners; debug stderr must contain `line 1`, `ambiguous`, and `_ or ()`.
  QA scenarios: happy: `cargo test parser_handles_greedy_rhs_and_disambiguation`; failure: run `cargo run -- --debug tests/fixtures/parser-left-ambiguity-error.dyyl`, evidence `.omo/evidence/task-3-dyyl-language-interpreter.txt` and `.stderr`.
  Commit: N | feat(parser): implement dyyl call grammar

- [x] 4. Implement value model, environment, sentinels, and debug diagnostics
  What to do / Must NOT do: Add `Value` variants for Num, Str, List, Dict, sentinel/empty helpers, command-specific sentinel precedence, global environment, immutable num/string rebinding via `set`, mutable list/dict containers, and structured internal errors converted to sentinels. Debug mode must write line number + command + reason to stderr.
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 5-12
  References: `dyyl-api-reference.md:27-34`, `dyyl-api-reference.md:85-87`, `.omo/drafts/dyyl-interpreter.md` decisions 3,11,26,27,28.
  Acceptance criteria: unit tests for global scope, `set`, container mutability, `dict.get/list.get` returning -1, debug warnings. Concrete failure fixture `tests/fixtures/runtime-sentinels.dyyl` includes `$missing`, missing `dict.get`, out-of-bounds `list.get`, and `unknown.command`; expected stdout contains four sentinel lines and debug stderr contains each command name and line number.
  QA scenarios: happy: `cargo test value_environment_and_sentinels`; failure: run `cargo run -- --debug tests/fixtures/runtime-sentinels.dyyl`, evidence `.omo/evidence/task-4-dyyl-language-interpreter.stderr`.
  Commit: N | feat(runtime): add values environment sentinels debug

- [x] 5. Implement CAS/display numeric layer
  What to do / Must NOT do: Build numeric abstraction on accepted CAS backend or fallback. Implement exact rationals, constants, roots, trig/log/pow, negative/fraction exponent, `math.approx` 15 significant digits, dyyl display (`⅓`, `1⅔`, `√2`, `(√2)/2`), Unicode char-code arithmetic for single-char string +/- integer. Do not depend on CAS default Display for dyyl output.
  Parallelization: Wave 2 | Blocked by: 2,4 | Blocks: 6,7,8
  References: `dyyl-api-reference.md:36-56`, `dyyl-api-reference.md:91-148`, `.omo/drafts/dyyl-interpreter.md` decisions 24,32,33,38-40,44,51-53.
  Acceptance criteria: tests cover every `math.*` command in the spec: `add`, `sub`, `multi`, `div`, `strike`, `surplus`, `pow`, `sqrt`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `log`, `ln`, `lg`, `exp`, `round`, `floor`, `ceil`, `pi`, `e`, `tau`, `approx`, and `hash`. Required edge assertions include `1/3 * 3 -> 1`, `5/3 -> 1⅔`, `√2/2 -> (√2)/2`, `sin(π/6) -> 1/2`, `cos(0) -> 1`, `math.strike -7, 2 -> -3`, `math.surplus -7, 2 -> -1`, `math.pow 2, 1/2 -> √2`, `math.round -0.5 -> -1`, `math.floor/ceil` on positive and negative values, constants display as `π`/`e`/`τ`, `math.approx π` has 15 significant digits, `math.hash` works for numeric and string values, `"a"+1 -> "b"`, and invalid char offset -> empty string.
  QA scenarios: happy: `cargo test math_all_commands_cas_display_and_char_arithmetic`; failure: fixture `tests/fixtures/math-failures.dyyl` covers division by zero, modulo by zero, mixed multi-char string+integer, non-integer char offset, and invalid Unicode scalar offset; expected outputs are documented sentinels and debug warnings, evidence `.omo/evidence/task-5-dyyl-language-interpreter.txt`.
  Commit: N | feat(math): implement dyyl numeric CAS display

- [x] 6. Implement core runtime commands: create, set, logic, and control flow
  What to do / Must NOT do: Implement `create.str`, `create.num`, `set`, `$var`, `logic.*`, conditions as numeric 1/0, `logic.if`, `logic.else`, `logic.while`, `logic.for`, execution-count returns, nested block span validation, and else linking only to the previous if. Must not implement break/continue or local scopes.
  Parallelization: Wave 2 | Blocked by: 2,3,4 | Blocks: 7,8
  References: `dyyl-api-reference.md:27-34`, `dyyl-api-reference.md:60-87`, `.omo/drafts/dyyl-interpreter.md` decisions 14,15,26,27,46.
  Acceptance criteria: tests cover every `logic.*` command: `un`, `and`, `or`, `same`, `not.same`, `more`, `less`, `more.same`, `less.same`, `max`, `min`, `between`, `clamp`, `is.num`, `is.str`, `is.empty`, `if`, `else`, `while`, and `for`. Also test true/false if, else previous-if semantics, while/for execution count, nested block N too small sentinel/debug warning. Concrete failure fixture `tests/fixtures/control-underdeclared-block.dyyl` contains an outer while whose declared N does not cover an inner while block; debug stderr must include `line`, `block span`, and `underdeclared`.
  QA scenarios: happy: `cargo test logic_all_commands_and_control_flow`; failure: run `cargo run -- --debug tests/fixtures/control-underdeclared-block.dyyl`, evidence `.omo/evidence/task-6-dyyl-language-interpreter.stderr`.
  Commit: N | feat(runtime): implement logic and control flow

- [x] 7. Implement CLI entry point and script runner
  What to do / Must NOT do: Add CLI handling for `dyyl <filename>` and `--debug`, arbitrary extension input, file loading, stdout/stderr behavior, nonzero process status only for host-level failures like unreadable script file. Runtime command errors stay sentinel-driven. Do not implement REPL.
  Parallelization: Wave 2 | Blocked by: 5,6 | Blocks: 8,13
  References: user request `dyyl filename`; `dyyl-api-reference.md`; `.omo/drafts/dyyl-interpreter.md` decisions 5,34.
  Acceptance criteria: `cargo run -- tests/fixtures/basic.anything` executes; `cargo run -- --debug tests/fixtures/errors-debug.dyyl` emits warnings.
  QA scenarios: happy: `cargo run -- tests/fixtures/basic.anything`, failure: missing script exits host-level error, evidence `.omo/evidence/task-7-dyyl-language-interpreter.txt`.
  Commit: N | feat(cli): add dyyl script runner

- [x] 8. Implement string commands and regex hybrid engine
  What to do / Must NOT do: Implement `str.*` commands with Unicode char indexing, `replace` first vs `replace.all`, split preserving empty elements, regex escape/unescape, hybrid `regex`/`fancy-regex`, `{N}` formatting, `str.to.num` failure -1, `str.from.num` dyyl display. Do not use byte offsets for user-visible indexing.
  Parallelization: Wave 3 | Blocked by: 5,6 | Blocks: 9-12
  References: `dyyl-api-reference.md:145-192`, `.omo/drafts/dyyl-interpreter.md` decisions 13,31,41,42.
  Acceptance criteria: tests cover every `str.*` command: `len`, `get`, `slice`, `find`, `rfind`, `count`, `replace`, `replace.all`, `insert`, `remove`, `upper`, `lower`, `capital`, `reverse`, `repeat`, `pad.left`, `pad.right`, `trim`, `trim.left`, `trim.right`, `split`, `join`, `start`, `end`, `contains`, `index`, `match`, `extract`, `replace.regex`, `escape`, `unescape`, `encode`, `decode`, `format`, `to.num`, `from.num`. Tests must include CJK/emoji length/get/slice/reverse, regex simple/advanced fallback, format indexes, split empty elements, encode/decode base64/url/hex, padding, trims, and missing-index `-1` behavior. Concrete failure fixture `tests/fixtures/string-invalid-regex.dyyl` contains `str.match "abc", "("`; debug stderr must include `regex` and line number.
  QA scenarios: happy: `cargo test string_all_commands_unicode_and_regex`; failure: run `cargo run -- --debug tests/fixtures/string-invalid-regex.dyyl`, evidence `.omo/evidence/task-8-dyyl-language-interpreter.txt`.
  Commit: N | feat(str): implement dyyl string commands

- [x] 9. Implement dict and list commands
  What to do / Must NOT do: Implement mutable dict/list containers, arbitrary dict key values, `dict.*`, `list.create/get/len/append/insert/remove/join/contains/index/reverse/sort/slice`, list sort rules, missing/out-of-bounds `-1`. Do not treat `mcm.*` as dict/list-related special cases.
  Parallelization: Wave 3 | Blocked by: 8 | Blocks: 13 | Can parallelize with: 10,11,12
  References: `dyyl-api-reference.md:189-247`, `.omo/drafts/dyyl-interpreter.md` decisions 10,11,25,28,43,49.
  Acceptance criteria: tests cover every dict/list command: `dict.create`, `dict.set`, `dict.get`, `dict.has`, `dict.del`, `dict.keys`, `dict.vals`, `dict.len`, `list.create`, `list.get`, `list.len`, `list.append`, `list.insert`, `list.remove`, `list.join`, `list.contains`, `list.index`, `list.reverse`, `list.sort`, and `list.slice`. Tests must cover dict string/num keys, dict.keys/vals list results, all list mutations, list.sort all-number/all-string/mixed ordering, `dict.get` missing `-1`, and `list.get` out-of-bounds `-1`. Concrete failure fixture `tests/fixtures/container-missing-access.dyyl` includes missing dict/list access and expected `-1` output.
  QA scenarios: happy: `cargo test containers_all_dict_list_commands`; failure: run `cargo run -- --debug tests/fixtures/container-missing-access.dyyl`, evidence `.omo/evidence/task-9-dyyl-language-interpreter.txt`.
  Commit: N | feat(data): implement dict and list commands

- [x] 10. Implement file and network commands with isolated tests
  What to do / Must NOT do: Implement absolute-path-only `file.write`, `file.append`, `file.read`, `net.get`, `net.download`. Use local temp dirs and a local HTTPS server with generated test certificate in tests; never rely on public internet. Relative paths return sentinel/debug. Do not downgrade the spec to plain HTTP.
  Parallelization: Wave 3 | Blocked by: 8 | Blocks: 13 | Can parallelize with: 9,11,12
  References: `dyyl-api-reference.md:288-310`, `.omo/drafts/dyyl-interpreter.md` decisions 29,37,48.
  Acceptance criteria: tests for write overwrite, append, read, relative path rejection, local HTTPS GET body string, download returning byte count, and failed HTTPS request returning command-specific sentinel. Test harness must generate a localhost cert and configure only the test client to trust it.
  QA scenarios: happy: `cargo test file_and_network_commands_local_https_only`; failure: run fixture `tests/fixtures/file-net-failures.dyyl` with relative path and failed HTTPS URL, evidence `.omo/evidence/task-10-dyyl-language-interpreter.txt`.
  Commit: N | feat(io): implement file and network commands

- [x] 11. Implement terminal IO commands with testable input abstraction
  What to do / Must NOT do: Implement `io.out`, `io.changeline`, `io.in`, `io.get`, `io.inpasswd`. Add test abstraction for stdin/key/password input so tests do not require human interaction. `io.get` returns key-name strings and no echo in real terminal mode. `io.out` is single-arg and uses dyyl display for numerics.
  Parallelization: Wave 3 | Blocked by: 8 | Blocks: 13 | Can parallelize with: 9,10,12
  References: `dyyl-api-reference.md:306-314`, `.omo/drafts/dyyl-interpreter.md` decisions 19,44,50.
  Acceptance criteria: unit tests for output formatting, input string, key names, password echo modes through mocked input.
  QA scenarios: happy: `cargo test terminal_io_with_mock_input`; failure: no input returns appropriate sentinel, evidence `.omo/evidence/task-11-dyyl-language-interpreter.txt`.
  Commit: N | feat(io): implement terminal commands

- [x] 11a. Fix bare nested zero-arity input commands in greedy arguments
  What to do / Must NOT do: Ensure bare command names such as `io.in` are parsed/evaluated as zero-arity command calls when used as arguments without quotes or escaping, so `/home/usr/a.dyyl` (`io.out io.in`) and `/home/usr/b.dyyl` (`set a,io.in`; `io.out $a`) output the user's stdin content instead of the literal string `io.in`. Quoted/escaped forms must still remain literal strings. Do not change quoted string semantics or require users to write parentheses for zero-arity input commands.
  Parallelization: Patch task | Blocked by: 11 | Blocks: 12,13
  References: user report `/home/usr/a.dyyl`, `/home/usr/b.dyyl`; `dyyl-api-reference.md` terminal IO syntax; parser greedy RHS rules.
  Acceptance criteria: automated tests cover `io.out io.in`, `set a,io.in` + `io.out $a`, and quoted literal `io.out "io.in"`; manual QA runs `cargo run -- /home/usr/a.dyyl` and `cargo run -- /home/usr/b.dyyl` with piped stdin and confirms stdout is the input content.
  Commit: N | fix(parser): evaluate bare zero-arity input commands in arguments

- [x] 12. Implement user/system/time commands
  What to do / Must NOT do: Implement `user.id`, `user.name`, `user.bash`, `system.os`, `system.arch`, and `time.*` commands. `user.config` must remain deleted/unimplemented. `user.bash` success returns stdout string and failure `-1`. `time.now` uses `YYYY-MM-DD HH:mm:ss`; weekday is 1=Monday.
  Parallelization: Wave 3 | Blocked by: 8 | Blocks: 13 | Can parallelize with: 9,10,11
  References: `dyyl-api-reference.md:318-356`, `.omo/drafts/dyyl-interpreter.md` decisions 18,21,30,35,45.
  Acceptance criteria: tests cover every user/system/time command: `user.id`, `user.name`, `user.bash`, `system.os`, `system.arch`, `time.get`, `time.now`, `time.year`, `time.month`, `time.day`, `time.hour`, `time.minute`, `time.second`, `time.weekday`, `time.weekday.name`, `time.format`, `time.diff`, and `time.add`. Tests must cover harmless `printf dyyl-ok`, failing bash command, os/arch string nonempty, `time.now` regex, weekday 1=Monday range, weekday.name nonempty, custom `time.format`, `time.diff`, and `time.add` arithmetic.
  QA scenarios: happy: `cargo test user_system_time_all_commands`; failure: fixture `tests/fixtures/user-bash-failure.dyyl` returns -1 for failing bash, evidence `.omo/evidence/task-12-dyyl-language-interpreter.txt`.
  Commit: N | feat(system): implement user system time commands

- [x] 13. Add golden fixture scripts and end-to-end CLI snapshots
  What to do / Must NOT do: Create fixtures for arithmetic, parser ambiguity, nested control flow, containers, IO/string, local HTTPS file/net test path, and errors-debug. Use snapshot assertions for stdout/stderr. Include fixture proving `mcm.*` is unknown sentinel and not handled. Do not rely on public network or human input.
  Parallelization: Wave 4 | Blocked by: 7-12 | Blocks: 14
  References: all updated `dyyl-api-reference.md`; Metis required tests.
  Acceptance criteria: `cargo test golden_fixtures` passes; `cargo run -- tests/fixtures/arithmetic.dyyl` outputs expected dyyl display.
  QA scenarios: happy: run all golden fixtures via `cargo test golden_fixtures`; failure: `--debug` fixture asserts warning contains command+line+reason, evidence `.omo/evidence/task-13-dyyl-language-interpreter.txt`.
  Commit: N | test(e2e): add dyyl golden fixtures

- [x] 14. Harden quality gates, docs alignment, and final cleanup
  What to do / Must NOT do: Run fmt, full tests, lints/diagnostics if available, review code for >250 LOC pure modules and split if needed, ensure no mcm handler/stub exists, ensure current `dyyl-api-reference.md` matches implemented behavior. Do not add new features beyond spec.
  Parallelization: Wave 4 | Blocked by: 13 | Blocks: Final
  References: `.omo/drafts/dyyl-interpreter.md`; `dyyl-api-reference.md`; Metis scope-creep warnings.
  Acceptance criteria: `cargo fmt --check`, `cargo test`, targeted CLI runs pass; search confirms no mcm handler; evidence file records all commands and exit codes.
  QA scenarios: happy: `cargo fmt --check && cargo test` plus CLI fixture runs; failure: deliberately unknown `mcm.game.install` fixture returns sentinel/debug not handler, evidence `.omo/evidence/task-14-dyyl-language-interpreter.txt`.
  Commit: N | chore(quality): finalize dyyl interpreter gates

## Final verification wave

> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.

- [x] F1. Plan compliance audit: reviewer checks every implemented command against `dyyl-api-reference.md` and `.omo/drafts/dyyl-interpreter.md` decisions, especially mcm exclusion and sentinel behavior.
- [x] F2. Code quality review: reviewer checks Rust module boundaries, error handling, parser complexity, tests, no scope creep, no oversized modules without split.
- [x] F3. Real manual QA: agent runs `cargo run -- tests/fixtures/*.dyyl` including arithmetic, nested control flow, containers, errors-debug, and local HTTPS net fixtures, and records stdout/stderr to `.omo/evidence/final-dyyl-language-interpreter.txt`.
- [x] F4. Scope fidelity: reviewer verifies no REPL, packaging, mcm implementation/stub, `/x/mcm` integration, or undocumented command was added.

## Commit strategy

- No commit is required unless the user explicitly asks for commits.
- If commits are requested later, use atomic commits by wave:
  - `feat(scaffold): initialize dyyl rust crate`
  - `feat(parser): implement dyyl command grammar`
  - `feat(runtime): implement values and control flow`
  - `feat(math): implement dyyl numeric CAS display`
  - `feat(commands): implement dyyl standard library commands`
  - `test(e2e): add dyyl golden fixtures`
  - `chore(quality): finalize dyyl gates`

## Success criteria

- `cargo fmt --check` passes.
- `cargo test` passes.
- `cargo run -- tests/fixtures/arithmetic.dyyl` emits exact expected dyyl numeric display.
- `cargo run -- --debug tests/fixtures/errors-debug.dyyl` emits sentinel outputs and stderr warnings with line+command+reason.
- Parser tests cover greedy RHS, parentheses, `_`/`empty`, inline comments, string-contained separators, and no continuation.
- CAS tests cover exact rationals, roots, constants, trig special values, display formatting, `math.approx`, and char-code arithmetic.
- Control-flow tests cover `if`, `else`, nested `while/for`, execution counts, and under-declared block spans.
- File/net tests use temp absolute paths and local HTTPS server only, with a generated test certificate trusted only by the test harness.
- `mcm.*` is not implemented and is handled only as unknown command sentinel/debug warning.
- No release packaging, REPL, formatter, LSP, mcm integration, or undocumented command is added.
