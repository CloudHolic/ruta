# Roadmap

Twelve stages from an empty workspace to a complete Lua 5.5.1. The order is chosen so that
each stage can be scored: every stage after 4 should move the number that `cargo test`
prints.

Current position: **stage 1 complete, stage 2 next. Conformance 0/31, parse 610/636.**

## Definition of done

| Milestone | Condition                                                                             |
| --------- | ------------------------------------------------------------------------------------- |
| v1.0      | Every file in the official test suite that does not depend on the C API passes        |
| v2.0      | C ABI embedding layer, the `T`-dependent files, and the remaining specification items |

## Stages

**0 — Recon and scoreboard.** Complete.
Workspace, vendored PUC-Lua 5.5.1 and the official suite, `cargo xtask build-reference`,
`conformance/manifest.toml`, and the differential harness. No interpreter code.

**1 — Lexer, parser, AST.** Complete.
Target: every scored file _parses_. This is not the same as passing — nothing executes yet —
but it is a real gate, because a parse error stops a file before any of it runs.

**This stage is scored against `luac -p`, not against the main scoreboard.** See "Stages 1-3
and the parse scoreboard" below.

Lua 5.5's `global` declaration is used by 17 files in the suite and is wider than it first
looks. `goto.lua` alone exercises:

- `global <const> *`, `global *`, `global none`
- initializers: `global<const> a, b, c = 10, 20, 30`, and `global a, b, c, d = table.unpack{...}`
  with the usual expression-list adjustment
- `global function foo (x)`, combining with the function statement
- `global X<close>`, which must be rejected
- redefinition (`global 'print' already defined`), which is a **runtime** error, not a
  compile-time one

A parser that does not know this syntax cannot read the suite at all.

`goto.lua:328` depends on `global` not being a reserved word. The mechanism is a build
option, not the test library: `luaconf.h` defines `LUA_COMPAT_GLOBAL`, on by default, which
keeps `global` usable as an ordinary identifier; `ltests.h` sets it to 0, so a reference built
with the test headers rejects `global = 1` while a normal one accepts it. The test checks `T`
because `T` correlates with that build, not because `T` causes it. **The flag ruta needs is
therefore `compat_global`, not "test mode"** — named after the option it actually mirrors.

It is always on for v1.0, and "on" only means that `global` is an ordinary name, so the lexer
carries no flag: it returns `global` as a name and the parser separates the two readings with
one token of lookahead. The flag is built at v2.0, where turning it off first has a consumer.

This file is also where the provenance requirement gets concrete:

```lua
checkerr([[
  global foo <const>;
  function foo (x)
    return
  end
]], "%:2%:")   -- correct line in error message
```

The suite asserts on the _line number_ inside a compile error, not just the text.

`literals.lua` and `constructs.lua` are the closest thing to direct tests of this stage.
Constraint 12 in [INVARIANTS.md](../INVARIANTS.md) binds the AST design and should be read
before starting.

**2 — Scope resolution pass.**
Locals, upvalue capture, `goto` and label scoping, and the `global` declarations from stage 1
resolved against their scopes. Targets: `locals.lua`, `goto.lua`, `closure.lua`.

Also scored against `luac -p`: most of what this stage produces is compile-time errors, and
their text and line numbers are comparable without running anything.

**3 — Value representation and heap interface.** No collector yet.
Tables as a hybrid of an array part and a hash part, written by hand. Strings, handles,
allocation.

**Neither scoreboard moves during this stage — not the conformance one, not the parse one.**
The only check is Rust unit tests written alongside the code, which makes this the easiest
stage to get wrong quietly and the one where test discipline has to be deliberate rather
than inherited from the harness.

[INVARIANTS.md](../INVARIANTS.md) constraints 1-6 all land here and are close to
irreversible afterward; read them first.

**4 — IR, code generation, VM core.**
The first stage that moves the conformance scoreboard. Register allocation, the dispatch
loop, calls and returns on an explicit frame stack. Targets: `calls.lua`, `constructs.lua`,
`vararg.lua`, `verybig.lua`, `cstack.lua`.
Constraints 7, 8, 10, 11, and 13 apply. `ruta-compile` and `ruta-bytecode` are split out
here, and constraint 13 has to hold from the commit that creates them.

**A call-depth limit is part of this stage's design, not an afterthought.** Unbounded
recursion has to surface as a catchable Lua error; without a limit it is an OOM or a crash,
which is a failed implementation rather than a missing feature. PUC-Lua needs two limits
because Lua-to-Lua calls live on its own `CallInfo` chain while calls through C functions and
metamethods really do recurse on the C stack (`LUAI_MAXSTACK` and `LUAI_MAXCCALLS`
respectively) — `cstack.lua` exists to check the second one degrades into an error instead of
a segfault. Constraint 8 puts both paths on the same explicit frame stack, so ruta needs one
limit rather than two. What must match PUC is the error text (`stack overflow`, and the
`error in error handling` case when the handler overflows too); the depth itself need not,
unless `cstack.lua` turns out to print one — check against the reference when the file first
runs.

**4b — Closures and upvalues.**
Split out because upvalue capture interacts with both the register allocator and the future
collector. Targets: `closure.lua`, `locals.lua`.

**5 — Numeric semantics and fuzzing.**
Integer and float subtypes, overflow and coercion rules, string-to-number conversion,
shortest round-trip formatting. Differential fuzzing over generated arithmetic expressions
starts here and reuses `differential(script: &str)` from the harness.
Targets: `math.lua`, `bitwise.lua`, `tpack.lua`, `sort.lua`.

**6 — Metatables, error handling, `<close>`.**
All metamethods, `pcall` and `error`, and to-be-closed variables. Error _messages_ matter as
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

## Stages 1-3 and the parse scoreboard

The conformance scoreboard cannot move before stage 4, because it compares program output and
nothing runs until there is a VM. That leaves three stages — plausibly months — with no
number, which is the same problem stage 0 existed to solve.

Stages 1 and 2 get their own oracle instead: **`luac -p`**, which parses without executing.
`cargo xtask build-reference --luac` builds it from the same vendored sources, and `ruta -p`
is the counterpart on this side. Accepted-versus-rejected and the exact text and line numbers
of compile errors are all comparable this way.

It scores 34 files rather than the conformance board's 31. The three skipped files are
skipped on grounds of what happens when they _run_ — exhausting memory, or being a module
rather than a test — and none of that stops them from parsing. Its second axis is the corpus:
the chunks the suite hands to `load`, which otherwise go unchecked until stage 4. What is in
the corpus and why is in `conformance/README.md`.

Stage 3 has no equivalent and is covered by Rust unit tests alone. That is a real gap, not an
oversight; it is recorded here so that the absence is noticed rather than assumed.

## On the target files

Stage 0 through 1 assignments are grounded in the suite investigation recorded in the
per-file notes in `conformance/manifest.toml`. The rest are expectations, not measurements.
The scoreboard is the authority: when a file passes at a different stage than listed here,
this document is what was wrong.
