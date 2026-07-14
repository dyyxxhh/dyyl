# dyyl

`dyyl` is a small scripting language and interpreter designed for MCM (Minecraft Manager) automation. It focuses on predictable command-style syntax, exact symbolic numeric computation, and an optional host protocol that lets scripts call back into an external launcher/manager such as MCM.

The project is licensed under the GNU Affero General Public License v3.0 or later. See [`LICENSE`](LICENSE).

## Status

`dyyl` is under active development. The current implementation includes a real lexer, parser, runtime, exact numeric layer, string/container helpers, file/network/user/system commands, tests, and an experimental MCM host protocol. The language surface is still evolving and compatibility is not yet guaranteed.

## Highlights

- Command-oriented scripting syntax with comma-separated arguments.
- `_` and `empty` placeholders for skipped argument positions.
- Parenthesized nested calls such as `set $i, math.add($i, 1)`.
- Exact unified numeric type via `create.num` for integers, fractions, roots, and constants.
- Human-friendly exact output such as `1⅔`, `√2`, `(√2)/2`, and symbolic expressions.
- String, logic, math, dictionary, list, file, network, terminal, user, and system APIs.
- `--host-json` mode for NDJSON request/response integration with an external MCM host.
- English and Chinese diagnostics via `--lang <en|zh>`.

## Requirements

- Linux or another Rust-supported platform.
- Rust stable toolchain with Cargo.
- Network access only for commands/tests that explicitly use network APIs.

## Install / build

```bash
git clone https://github.com/dyyxxhh/dyyl.git
cd dyyl
cargo build --release
```

The release binary is written to:

```text
target/release/dyyl
```

For local development you can also run directly through Cargo:

```bash
cargo run -- ./examples/script.dyyl
```

## Quick start

Create `hello.dyyl`:

```dyyl
io.out "Hello from dyyl"
create.num i
set $i, math.add(1/3, 5/3)
io.out $i
```

Run it:

```bash
cargo run -- hello.dyyl
# or, after cargo build --release:
./target/release/dyyl hello.dyyl
```

Expected output includes:

```text
Hello from dyyl
2
```

## CLI usage

```bash
dyyl [--debug] [--host-json] [--lang <en|zh>] <filename>
```

Options:

- `--debug`: print parser/runtime errors with line number, error kind, and offending command text.
- `--host-json`: enable the streaming MCM host protocol. `mcm.*` commands are emitted as NDJSON to stdout, and the host responds with NDJSON on stdin.
- `--lang <en|zh>`: choose diagnostic language. If no filename is supplied, this saves the preferred language to the user config.
- `<filename>`: script file to execute. Any extension is accepted; `.dyyl` is conventional.

Arguments after `<filename>` are passed verbatim to the script and can be read via `cli.*` commands (see [Script command-line arguments](#script-command-line-arguments)). The interpreter does not interpret these args itself.

Examples:

```bash
dyyl script.dyyl
dyyl --debug script.dyyl
dyyl --lang zh script.dyyl
dyyl --host-json pack.dyyl
```

## Script command-line arguments

A dyyl script can read command-line arguments passed after the filename via the `cli.*` command family. The first line may be a shebang (`#!/usr/bin/env dyyl`) since `#` starts a comment.

| Command | Args | Returns | Semantics |
|---|---|---|---|
| `cli.args` | none | list of strings | all args after filename, in order |
| `cli.count` | none | number | arg count |
| `cli.get <idx>` | one non-negative integer | string or `-1` | 0-based index access; OOB/negative returns `-1` |
| `cli.has <flag>` | one string | `1` or `0` | exact flag match, or `--flag=...` form counts; no prefix matching |
| `cli.value <flag>` | one string | string or `empty` | value for `--flag value` or `--flag=value`; first occurrence wins; `empty` if not found or no value |
| `cli.script_name` | none | string | basename of the script file |

Example `a.dyyl` (no indentation, per dyyl style):

```dyyl
#!/usr/bin/env dyyl
logic.if cli.has("--help"), _
io.out "Usage: a.dyyl [--help] [--out FILE]"
logic.end
logic.if cli.has("--out"), _
io.out str.format("output: {0}", cli.value("--out"))
logic.end
```

Run: `./a.dyyl --help` or `dyyl a.dyyl --out result.txt`.

## Language basics

### Syntax rules

- Each command is one line. Line continuations are not supported.
- `#` starts a comment.
- The first line may be a shebang (`#!/usr/bin/env dyyl`); it is treated as a comment.
- Arguments at the same level are separated by commas.
- `_` and `empty` mean an intentionally empty argument position.
- Nested command arguments can be delimited with parentheses.
- Bare string arguments do not need quotes unless they contain commas or other special characters.
- Double-quoted strings support escapes such as `\n`, `\t`, `\\`, and `\"`.

```dyyl
# Right-hand side is greedy: math.add and its arguments form the value assigned to i.
set $i, math.add $i, 1

# Equivalent explicit nesting.
set $i, math.add($i, 1)

# Placeholder disambiguates argument positions.
logic.or math.add(a, b), _, c

# Strings.
io.out hello world
io.out "hello world"
io.out hello\, world
io.out "line one\nline two"
```

### Variables

```dyyl
create.str name
set $name, "Steve"
io.out $name

create.num count
set $count, math.add(1, 2)
io.out $count
```

Variable commands:

| Command | Purpose |
|---|---|
| `create.str name` | Create a string variable initialized to `empty`. |
| `create.num name` | Create a numeric variable initialized to `0`. |
| `set $name, value` | Assign a value. |
| `$name` | Read a value. |

### Exact numbers

`create.num` stores values symbolically where possible. Fractions and roots remain exact until explicitly approximated.

```dyyl
create.num x
set $x, math.div(√2, 2)
io.out $x
io.out math.approx($x)
```

Output prefers exact forms before decimal approximations.

## API reference

The complete API reference is maintained in [`dyyl-api-reference.md`](dyyl-api-reference.md). It documents:

- syntax conventions
- variables and numeric representation
- logic/control flow
- math, trigonometry, logarithms, constants, and hashing
- string operations
- dictionaries and lists
- IO, file, network, terminal, user, and system commands
- MCM host commands
- return values and sentinel values

## MCM host protocol

When `--host-json` is enabled, dyyl can call `mcm.*` commands through a host process instead of implementing launcher operations itself. The interpreter writes NDJSON requests to stdout and reads NDJSON responses from stdin. Diagnostics remain on stderr.

This mode is intended for integration with MCM package/build workflows, where a dyyl script describes operations and MCM supplies trusted host-side behavior.

## Development

Run the test suite:

```bash
cargo test
```

Run Clippy with the project lint policy:

```bash
cargo clippy --all-targets --all-features
```

Format code:

```bash
cargo fmt
```

Build a release binary:

```bash
cargo build --release
```

## Repository layout

```text
src/
  lexer/       tokenization and lexical rules
  parser/      command parsing, arity handling, nesting rules
  math/        exact numeric display and operations
  runtime/     execution engine, values, IO, host providers
  config.rs    user configuration
  i18n.rs      localized diagnostics
tests/
  fixtures/    golden dyyl scripts
  *.rs         parser/runtime/math/string/container/host tests
```

## License

`dyyl` is free software under the GNU Affero General Public License v3.0 or later. If you modify and run the software as a network service, AGPLv3 requires that users interacting with that service can receive the corresponding source code.