# Roadmap

Twelve stages from an empty workspace to a complete Lua 5.5.1. The order is chosen so that
each stage can be scored: every stage after 4 should move the number that `cargo test`
prints.

Current position: **stage 0 complete, stage 1 next. Scoreboard 0/31.**

## Definition of done

| Milestone | Condition |
|---|---|
| v1.0 | Every file in the official test suite that does not depend on the C API passes |
| v2.0 | C ABI embedding layer, the `T`-dependent files, and the remaining specification items |

## Stages

**0 — Recon and scoreboard.** Complete.
Workspace, vendored PUC-Lua 5.5.1 and the official suite, `cargo xtask build-reference`,
`conformance/manifest.toml`, and the differential harness. No interpreter code. The exit
condition was a scoreboard printing `0/31`, which is what a correct harness reports against
an empty implementation. See [testing.md](testing.md).

**1 — Lexer, parser, AST.**
Target: every scored file *parses*. This is not the same as passing — nothing executes yet —
but it is a real gate, because a parse error stops a file before any of it runs.

Two things bite here. First, Lua 5.5's `global` declaration is used by 17 files in the suite,
in several forms: `global <const> *`, `global none` followed by individual declarations, and
`global <const> a, b`. A parser that does not know this syntax cannot read the suite at all.
Second, `goto.lua:328` depends on `global` being a reserved word only when the `T` library is
present — the lexer has to reproduce that distinction.

`literals.lua` and `constructs.lua` are the closest thing to direct tests of this stage.
Constraint 12 in [INVARIANTS.md](../INVARIANTS.md) binds the AST design and should be read
before starting.

**2 — Scope resolution pass.**
Locals, upvalue capture, `goto` and label scoping, and the `global` declarations from stage 1
resolved against their scopes. Targets: `locals.lua`, `goto.lua`, `closure.lua`.

**3 — Value representation and heap interface.** No collector yet.
Tables as a hybrid of an array part and a hash part, written by hand. Strings, handles,
allocation.

No test file moves during this stage, which makes it the easiest one to get wrong quietly.
[INVARIANTS.md](../INVARIANTS.md) constraints 1-6 all land here and are close to
irreversible afterward; read them first.

**4 — IR, code generation, VM core.**
The first stage that moves the scoreboard. Register allocation, the dispatch loop, calls and
returns on an explicit frame stack. Targets: `calls.lua`, `constructs.lua`, `vararg.lua`,
`verybig.lua`, `cstack.lua`.
Constraints 7, 8, 10, and 11 apply.

**4b — Closures and upvalues.**
Split out because upvalue capture interacts with both the register allocator and the future
collector. Targets: `closure.lua`, `locals.lua`.

**5 — Numeric semantics and fuzzing.**
Integer and float subtypes, overflow and coercion rules, string-to-number conversion,
shortest round-trip formatting. Differential fuzzing over generated arithmetic expressions
starts here and reuses `differential(script: &str)` from the harness.
Targets: `math.lua`, `bitwise.lua`, `tpack.lua`, `sort.lua`.

**6 — Metatables, error handling, `<close>`.**
All metamethods, `pcall` and `error`, and to-be-closed variables. Error *messages* matter as
much as error behavior — the suite asserts on their exact text.
Targets: `events.lua`, `errors.lua`, `locals.lua`.
Constraint 9 applies.

**7 — Coroutines.**
Yield from arbitrary call depth, which constraint 8 exists to make possible.
Targets: `coroutine.lua` (through line 1054; the rest is v2.0), `big.lua`.

**8 — Garbage collection.**
In order: mark-sweep, weak tables, ephemerons, finalizers, then incremental and generational
modes. Targets: `gc.lua`, `gengc.lua` (through line 126), `nextvar.lua`.
Constraints 1, 4, and 5 are what make this stage possible rather than a rewrite; re-read them
before starting.

**9 — Standard library long tail.**
`string`, `table`, `os`, `io`, `utf8`. Pattern matching is implemented by hand — it is not a
regex engine and cannot be delegated to one.
Targets: `strings.lua`, `pm.lua`, `utf8.lua`, `files.lua`, `main.lua`.

**10 — `dump`/`undump` and the `debug` library.**
Bytecode serialization round-trips and full introspection. Constraints 10 and 11 reserved the
space this stage needs. Targets: `db.lua`, `all.lua` — `all.lua` matters here specifically
because its `dofile` round-trips every file through `string.dump` and `load`, so it exercises
more than the files do individually.

**--- v1.0 ---**

**11 — C ABI embedding layer.**
The `lua_*` surface, the `T` library's requirements, and `longjmp`-based error propagation
swapped in behind the single path constraint 9 reserved.
Targets: `api.lua`, `code.lua`, `memerr.lua` — the three files in the `v2.0` tier.
See [ADR 0004](decisions/0004-defer-c-abi.md).

**--- v2.0 ---**

## On the target files

Stage 0 through 1 assignments are grounded in the suite investigation recorded in
[testing.md](testing.md) and in the per-file notes in `conformance/manifest.toml`. The rest
are expectations, not measurements. The scoreboard is the authority: when a file passes at a
different stage than listed here, this document is what was wrong.
