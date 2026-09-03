# Roadmap

Fourteen stages from an empty workspace to a Lua 5.5.1 that compiles to a standalone
executable. The order is chosen so that each stage can be scored: every stage after 4 should
move the number that `cargo test` prints.

Current position: **stage 3 complete, stage 4 next. Conformance 0/31, parse 635/636.**

## Definition of done

| Milestone | Condition                                                                             |
| --------- | ------------------------------------------------------------------------------------- |
| v1.0      | Every file in the official test suite that does not depend on the C API passes        |
| v2.0      | C ABI embedding layer, the `T`-dependent files, and the remaining specification items |
| v3.0      | Standalone executables, and native code generation behind the same IR                 |

Each milestone is a place the project can stop and still be a finished thing. v1.0 is a Lua
that runs the language; v2.0 is one that C programs can embed and that C extensions can load;
v3.0 is one that produces a binary.

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
Constraint 12 in [INVARIANTS.md](../INVARIANTS.md) binds the AST design.

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
Constraints 7, 8, 10, 11, 13, and 14 apply. `ruta-compile` and `ruta-bytecode` are split out
here, and constraint 13 has to hold from the commit that creates them.

**This is the stage v3.0 reaches back into.** The IR designed here is the boundary a native
backend attaches to nine stages later, and constraint 14 records what that costs now: native
frames share the explicit frame stack, and root discovery cannot be written in terms of VM
frames alone. Neither is expensive to reserve. Both are a rewrite to retrofit.

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
Constraints 1, 4, 5, and 14 are what make this stage possible rather than a rewrite; re-read
them before starting.

**9 — Standard library long tail.**
`string`, `table`, `os`, `io`, `utf8`, and `package`. Pattern matching is implemented by hand
— it is not a regex engine and cannot be delegated to one.
Targets: `strings.lua`, `pm.lua`, `utf8.lua`, `files.lua`, `main.lua`.

**10 — `dump`/`undump` and the `debug` library.**
Bytecode serialization round-trips and full introspection. Constraints 10 and 11 reserved the
space this stage needs. Targets: `db.lua`, `all.lua` — `all.lua` matters here specifically
because its `dofile` round-trips every file through `string.dump` and `load`, so it exercises
more than the files do individually.

**--- v1.0 ---**

**11 — C ABI embedding layer.**
The `lua_*` surface, the `T` library's requirements, and `longjmp`-based error propagation
swapped in behind the single path constraint 9 reserved. `package.loadlib` and the C searcher
land here too, which is what makes real extension modules — `lpeg`, `luasocket` — loadable.
Targets: `api.lua`, `code.lua`, `memerr.lua` — the three files in the `v2.0` tier.
See [ADR 0004](decisions/0004-defer-c-abi.md).

**--- v2.0 ---**

**12 — Standalone executables.**
Bundling several Lua files, and the runtime, into one binary. See "What v3.0 is" below for
why this is a smaller job than it sounds.

**13 — Native code generation.**
Lowering the IR to machine code, behind the boundary stage 4 established. See below.

**--- v3.0 ---**

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

## What v3.0 is

PUC-Lua ships an interpreter that reads scripts. `ruta` should also be able to hand you a
binary. Two separate things have to be true for that, and they are worth keeping apart
because one is nearly free and the other is most of a compiler.

### Stage 12 — bundling, not linking

There is nothing to link. In C, a translation unit that calls `bar()` leaves an unresolved
symbol for the linker to fill in; in Lua, `require "m"` is an ordinary runtime call that
consults `package.loaded`, searches `package.path`, reads a file, compiles it, runs it, and
caches the result. No cross-module reference survives to compile time, so there is nothing
for a linker to resolve.

What replaces it is `package.preload`. Compile each file to a chunk, install it under the
module name, and `require` finds it before it ever touches the filesystem. That is what
`luastatic` does, and it needs stage 9's `package` library and stage 10's serialization and
very little else.

The one hard part is deciding what to bundle. `require` takes an expression, and a program
can compute a module name at runtime. Following string literals covers ordinary code;
anything else has to be listed explicitly. Every tool in this space has the same limit —
`deno compile`, PyInstaller — and the answer is to document it rather than to guess.

**Statically linking C extension modules is an opt-in.** Dynamic loading, which stage 11
already provides, asks nothing of the user: an already-built `lpeg.dll` opens at runtime.
Linking one _into_ the executable means compiling it, which means a C toolchain on the
machine doing the build. That is the only place `ruta` asks for one, and it stays behind a
flag so that not asking remains the default.

### Stage 13 — native code generation

**This is not a performance feature, and the roadmap should not be read as promising one.**

Lua is dynamically typed with metatable-driven operators, so `a + b` compiles to a chain of
checks — both integers, either a float, a string that coerces, an `__add` metamethod, or an
error naming the variable — and native code has to inline all of it. What disappears is
dispatch overhead. The type checks stay. AOT compilation of a dynamic language without type
feedback does not produce the multiples people expect from the word "native"; that is what
tracing JITs like LuaJIT exist to get, by observing types at runtime rather than guessing at
compile time.

What stage 13 is actually for is the pipeline: attaching a code generator to a typed IR,
defining a calling convention and frame layout, and scanning roots precisely in native frames.

**The VM does not go away.** `load`, `dofile`, `require`, and `string.dump` are all in the
specification, so the compiler and the interpreter both ship inside the executable, and code
compiled at runtime keeps running on the VM. Native and interpreted frames therefore call
each other in both directions and share a heap and a collector. Making that boundary coherent
is the real content of this stage — see constraint 14, which is why it is reserved at stage 4
rather than discovered here.

Order within the stage: lower straight-line arithmetic and control flow first, then calls
across the native/VM boundary, then root maps, then the rest.

## On the target files

Stage 0 through 1 assignments are grounded in the suite investigation recorded in the
per-file notes in `conformance/manifest.toml`. The rest are expectations, not measurements.
The scoreboard is the authority: when a file passes at a different stage than listed here,
this document is what was wrong.
