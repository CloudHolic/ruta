# ruta

A reimplementation of **Lua 5.5.1** in Rust, targeting the full language specification and
standard library.

## Why

This is a learning project, and that shapes everything about it.

The goal is not another working Lua — there is an excellent one already, and it is vendored
in this repository as the reference oracle. The goal is to build a **bytecode VM, a garbage
collector, and a code generator** by hand, because building them is the only way to
understand them.

That is also why the dependency policy is severe. Hash maps, arenas, parser generators, and
float formatters all exist as mature crates, and every one of them would remove something
this project exists to do. `ruta-syntax` and `ruta-runtime` take **no dependencies at all**.
See [ADR 0005](docs/decisions/0005-zero-dependencies.md).

## Status

**Stage 0 of 12 complete.** The scoreboard reads:

```
ruta conformance - Lua 5.5.1

  v1.0        0/28
  v2.0        0/3
  impossible  -/0

  total       0/31
  skipped     3
```

`0/31` is the correct result at this point: the harness is finished and the interpreter is
not started. `ruta`'s `main` currently does nothing, and a scoreboard that gave it any credit
for that would be broken. See [docs/roadmap.md](docs/roadmap.md) for what comes next.

| Milestone | Condition |
|---|---|
| **v1.0** | Every file in the official test suite that does not depend on the C API passes |
| **v2.0** | C ABI embedding layer, the `T`-dependent files, and the remaining specification items |

## Building

Requires a Rust toolchain and a C compiler (MSVC, MinGW, or GCC/Clang — the reference build
detects what is available).

```bash
cargo xtask build-reference
```

Compiles the vendored PUC-Lua 5.5.1 into `target/reference/lua[.exe]`. This is the oracle
every test compares against, so it comes first.

```bash
cargo xtask build-reference --luac
```

Builds `luac` from the same sources. It parses without executing, and is the oracle for the
parse scoreboard that stages 1 through 3 are measured against.

```bash
cargo test
```

Runs the differential test suite and prints the scoreboard above, followed by the parse
scoreboard. Both exit 0 even when everything mismatches — the numbers are progress metrics,
not pass/fail gates.

```bash
cargo run --bin ruta -- script.lua
```

## How it is tested

There are no hand-written expectation tests. Every case runs the same input through PUC-Lua
and through `ruta` and compares stdout, stderr, and exit code byte for byte. Using the
reference as an oracle means the parts of Lua's behavior that the manual does not specify get
checked too.

The number counts **agreement with the reference, not absolute passes** — a file that fails
identically on both sides counts. [docs/testing.md](docs/testing.md) explains how to read the
scoreboard, what the tiers mean, and where the two platforms diverge.

## Architecture

```
source -> lexer -> parser -> AST -> scope resolution -> IR -> register allocation -> bytecode -> VM
```

PUC-Lua is a single-pass compiler with no AST. `ruta` inserts an AST and an IR on purpose, so
that each pass can be read and observed on its own —
[ADR 0002](docs/decisions/0002-ast-and-ir.md).

| Path | Contents |
|---|---|
| `crates/ruta-syntax/` | lexer, parser, AST, scope resolution |
| `crates/ruta-runtime/` | value representation, heap, GC, VM, standard library |
| `crates/ruta-cli/` | the `ruta` binary |
| `crates/ruta-conformance/` | differential test harness |
| `vendor/` | PUC-Lua sources, the official test suite, the manual — never modified |
| `conformance/` | scoring inputs: the test manifest, the `load` prelude, the parse corpus |
| `docs/` | roadmap, testing guide, decision records |

[INVARIANTS.md](INVARIANTS.md) lists the thirteen design constraints that cannot be reversed
later. They are worth reading before stages 3, 4, and 8 in particular.

## License

MIT. See [LICENSE](LICENSE).

Lua is also MIT-licensed. Vendored third-party material and its provenance are recorded in
[NOTICE](NOTICE) and `vendor/README.md`.
