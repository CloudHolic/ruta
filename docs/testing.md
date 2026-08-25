# Testing

`ruta` has no hand-written expectation tests. Correctness is measured by running the same
input through PUC-Lua 5.5.1 and through `ruta` and comparing what comes out. The reference
implementation is the oracle, which means behavior the specification leaves unstated gets
checked too — and there is a lot of it.

The scoreboard this produces is the project's only progress metric.

## Running it

```bash
cargo xtask build-reference
```

Compiles the vendored PUC-Lua 5.5.1 sources into `target/reference/lua` (`lua.exe` on
Windows). It uses the `cc` crate rather than Lua's makefile, so MSVC, MinGW, and POSIX
toolchains are all handled by the same code path. It is a no-op once the binary exists.

```bash
cargo test
```

Runs all 31 scored cases and prints the scoreboard. Takes roughly 29 seconds on Windows.
Without a reference binary it stops immediately:

```
conformance: no reference interpreter at <path> - run `cargo xtask build-reference`
```

**`cargo test` exits 0 even when every case mismatches.** Only a setup error — a missing
reference, a malformed manifest — exits nonzero. The scoreboard is a progress indicator, not
a gate; making it a gate would leave CI red until v1.0 and bury the signal it exists to
carry. Regression detection needs a recorded baseline and will be added once the number
leaves 0.

## Reading the scoreboard

```
ruta conformance - Lua 5.5.1

  v1.0        0/28
  v2.0        0/3
  impossible  -/0

  total       0/31
  skipped     3
```

**The number counts agreement with the reference, not absolute passes.** If a file fails
under PUC-Lua on this platform and `ruta` fails identically — same stdout, same stderr, same
exit code — that counts. `files.lua` and `main.lua` both fail on Windows for reasons that
have nothing to do with `ruta`, and both are still meaningful differential cases.

That distinction is what makes the metric usable this early. The alternative would require
knowing what each file _should_ print, which is exactly the knowledge the project does not
have yet.

### Checking that the harness itself is sound

Two numbers, always read together:

| Configuration              | Expected |
| -------------------------- | -------- |
| reference vs. reference    | 31/31    |
| reference vs. empty `ruta` | 0/31     |

The first alone is not enough. An early version of the comparison logic scored 31/31 against
itself and **4/31 against a `ruta` whose `main` did nothing**, because comparing only exit
codes let four files that exit 0 on both sides count as matches. An implementation that does
nothing must score zero, or the scoreboard is lying.

Once `ruta` starts producing output the first row can no longer be observed directly. When
changing comparison logic, put the reference on both sides and confirm 31/31 before trusting
any other number.

## The parse scoreboard

The conformance scoreboard cannot move before there is a VM, which leaves stages 1 through 3
without a number. Those stages get a second oracle: `luac -p`, which parses without executing.

```bash
cargo xtask build-reference --luac
cargo test --test parse
```

```
ruta parse - Lua 5.5.1

  files        0/34
  corpus       34/603

  total        34/637
```

**files** is every `.lua` in the suite — all 34, not the conformance board's 31. The three
files that board skips are skipped for runtime reasons: `heavy.lua` exhausts memory on
purpose, and `tracegc.lua` and `bwcoercion.lua` are modules other tests require rather than
tests themselves. `-p` never runs anything, so none of those reasons survive into parsing.

**corpus** is `conformance/parse-corpus/`: the strings the suite passes to `load`. These are
the Lua team's own curated compile-error cases — `checkerr("global X<close>", "cannot be")`
and several hundred more — and they normally only execute when a file runs, which is stage 4.
`cargo xtask extract-parse-corpus` brings them forward, running the suite under a prelude that
intercepts `load` and dumps every string chunk. Rerunning it reproduces the same files byte
for byte.

Extraction is selection, not collection. Taken as it comes, the suite yields over 100000
distinct chunks and 33 MB, almost all of it `constructs.lua` permuting the same few shapes.
Three limits cut it to 599:

| Limit           | Value   | Why                                                                                                        |
| --------------- | ------- | ---------------------------------------------------------------------------------------------------------- |
| chunk size      | 4 KB    | Larger ones are generated stress sources, and each tests a code-generation limit that stage 1 cannot reach |
| per source file | 100     | Process startup is 32 ms per side, so corpus size _is_ the scoreboard's running time                       |
| bytecode        | dropped | `all.lua` round-trips every file through `string.dump`; undumping is stage 10                              |

Chunks are deduplicated by content across the whole corpus, and `all.lua` is not extracted
from at all — it re-runs the whole suite, so everything it captures belongs to another file
and would be filed under a name that says nothing about where it came from.

The **corpus** count spans a second directory, `conformance/parse-cases/`, which holds four
files written by hand. Selection is what makes them necessary: the per-file limit keeps 30 of
`literals.lua`'s 42 lexical errors, and four messages survive only in the twelve it drops. The
two directories are separate because extraction empties the first one before rewriting it, so
nothing hand-written can live there. Both are scored on the same axis, and neither records an
expected output — `luac -p` decides every case as it runs.

The soundness pair is the conformance board's, with the same discipline: `luac -p` against
itself scores 34/34 and 603/603, and against a `ruta -p` that does nothing but fail it scores 0. **The stub has to fail loudly for the second number to mean anything** — one that exited 0
silently would match the reference on every file that parses, which is most of them.

Byte-for-byte means the line ending too. On Windows the reference writes CRLF, because its
streams go through the C runtime in text mode, and `ruta` matches it — see
[decisions/0006](decisions/0006-windows-line-endings.md). Until it did, the board sat at 0 with
every message otherwise correct.

The run takes about 70 seconds, nearly all of it process startup: two processes per case at
32 ms each. Parse-only runs reuse their sandbox instead of wiping it, since nothing is written
but the script itself; that is worth about 10 ms per case, and nothing else is left to trim
short of running cases in parallel.

What this scoreboard cannot check is listed in
[errors.md](errors.md#what-ruta-cannot-reproduce-in-stage-1).

## Tiers

Set per file in `conformance/manifest.toml`, which is the conformance harness's only input.

| Tier         | Meaning                                                               | Count |
| ------------ | --------------------------------------------------------------------- | ----- |
| `v1.0`       | Runs meaningfully without the `T` library                             | 28    |
| `v2.0`       | Returns early when `T == nil`; needs the C ABI layer to mean anything | 3     |
| `impossible` | Cannot be compared differentially even after v2.0                     | 0     |

The classification test is mechanical: **does the file take an early `return` when `T` is
nil?** `api.lua`, `code.lua`, and `memerr.lua` do — all 1416, 504, and 309 lines
respectively sit behind that guard, and without `T` each prints three lines and stops. Files
like `gc.lua` and `coroutine.lua` have `if T` blocks scattered through them instead, so the
rest of the file still runs; those are `v1.0` with a note recording which region is not being
checked.

**`impossible` is empty, and that is a real result rather than an oversight.** The tier is
reserved for files that cannot be compared even once v2.0 is complete. `T` does not qualify —
but the reason is narrower than "compile `vendor/lua-tests/ltests/` into the reference."

`ltests.h` shows that `T` is two things at once. One is a Lua library (`lua_checkmemory`,
a deliberately failable allocator, internal object dumps), and it reaches PUC-Lua's internals
rather than its public C API — `lua_printobj` takes a `GCObject *`. The other is a build
configuration: assertions on, jump tables off, `LUAI_MAXSTACK` cut to 68000, `LUAI_MAXCCALLS`
to 180, `LUAL_BUFFERSIZE` to 23, the string table minimum to 2. Every size is shrunk so that
ordinary programs hit boundaries that a normal build would never reach.

So the three `v2.0` files need the C ABI layer **and a ruta-side equivalent of `T`** — heap
verification, an allocator that fails on command, internal dumps — **and a build profile with
the same shrunken constants**. That is reimplementation, not reuse: what `T` exposes is
PUC-Lua's internals, and ruta does not have PUC-Lua's internals.

Deferred, then, rather than unreachable. The tier is kept because a file could still land in
it, and an empty tier is an honest thing to print.

Three files are skipped and excluded from the denominator: `heavy.lua`, a stress test
`all.lua` never invokes which deliberately exhausts memory and runs past the timeout, and
`tracegc.lua` and `bwcoercion.lua`, which are modules other tests `require` rather than tests
themselves.

## What the harness compares

stdout, stderr, and the exit code, byte for byte. `.gitattributes` pins line endings on the
vendored files so that this comparison means what it says. There are two deliberate
exceptions.

### The interpreter's own path

Lua prefixes runtime error messages with `argv[0]`:

```
E:\Projects\ruta\target\reference\lua.exe: files.lua:467: cannot open file '/dev/null'
```

The reference and `ruta` live at different paths, so without handling this **every case that
ends in an error would mismatch for that reason alone**. Each side's own path string is
replaced with a fixed token before comparison — a byte substitution of a known string, not a
regex or a heuristic. The prefix itself is still compared, so an implementation that omits it
is still caught.

### Files with nondeterministic output

Six files produce different output on every run. They are marked `nondeterministic = true`
in the manifest and compared by exit code **and the line count of each stream**.

| File             | Cause                                             |
| ---------------- | ------------------------------------------------- |
| `all.lua`        | prints the result of `math.randomseed()`          |
| `constructs.lua` | `math.random` selects which branch is tested      |
| `main.lua`       | prints an `os.tmpname()` path                     |
| `math.lua`       | prints the result of `math.randomseed()`          |
| `nextvar.lua`    | prints a random table seed                        |
| `sort.lua`       | random data changes timings and comparison counts |

`sort.lua` is why stripping the printed seed is not enough: the random data itself differs, so
the computation diverges rather than just the reporting.

Line counts are the weakest invariant that still has content — "the file took the same path
and printed the same number of times." They were stable across three runs on Windows
(`all` 370/7, `constructs` 8/0, `main` 5/6, `math` 13/0, `nextvar` 16/0, `sort` 10/0). Byte
lengths are not used, since the digit count of a random value moves them.

## Sandboxing

The suite writes to its working directory — `all.lua` creates `time.txt`, `files.lua` creates
temporary files. Since `vendor/` is never modified for any reason, nothing ever runs there.

Each case gets a fresh copy of the suite under `target/conformance/`, and the reference and
the candidate get **separate directories** — sharing one would let the first run's leftovers
change the second's results. Each case is given 120 seconds; `all.lua` alone takes 6.66
seconds under the reference, and an unbounded wait is a failure mode that hangs forever
rather than reporting.

## How the suite runs standalone

The harness runs individual files, but `all.lua` is written to run them as a batch. Whether
that transfers was checked before the harness was designed, by running the reference
interpreter directly.

### `all.lua` as a whole

On Windows:

| Condition                         | Result                                                             |
| --------------------------------- | ------------------------------------------------------------------ |
| as-is                             | stops at `main.lua:41`, `attempt to index a nil value (local 'f')` |
| `_port=true`                      | passes 28 files, stops at `files.lua:467` on `/dev/null`           |
| `_port=true`, flush block removed | `final OK !!!`, exit 0, 6.66 s                                     |

The "testing flush" block at `files.lua:466-480` uses `/dev/null` and `/dev/full` with **no
`_port` guard**, unlike the other Unix-only blocks in the same file (lines 718, 769, 858,
894). On Windows it is the single thing standing between the suite and a clean run.

### Individual files: the premise holds

Of the 33 files other than `all.lua`, **28 pass standalone with no setup at all.** Adding
`_port=true` brings in two more.

The globals `all.lua` installs — `_soft`, `_nomsg`, `T`, `ARG`, `Message` — turn out to be
harmless when absent. They are all read defensively; `Message` in particular is always called
as `(Message or print)(...)`. **No prelude file was needed and none was written.**

Two files need something other than plain invocation:

- `big.lua` must be wrapped in `coroutine.wrap` the way `all.lua` does it, or it fails with
  `attempt to yield from outside a coroutine`. This is the manifest's `recipe = "coroutine"`.
- `attrib.lua` and `files.lua` need `_port=true`, set as `port = true` in the manifest.

### Options are not there to make files pass

`main.lua` would pass with `port = true`. It is deliberately left off, because with `_port`
set the file returns at line 6 and executes nothing — a pass worth zero information. Without
it, the file runs to line 41 and 40 lines' worth of behavior gets compared against the
reference, which fails at the same place.

`strings.lua` is the same trade in miniature: `_port` would cut its output from 14 lines to
12 by skipping tests. Also left off.

**An option exists only to make more of a file execute.** Since a shared failure counts as a
match, making a file pass is not by itself worth anything.

## Platform differences

Both Windows and Linux are supported targets; Windows is the primary development
environment. There is no CI — the Linux side is verified by running the harness under WSL,
and this section is where those observations are recorded.

That is a deliberate choice rather than an omission. Because the harness always exits 0, a CI
job would be checking little more than that the workspace compiles, which a local build
already covers. The genuinely platform-dependent code — the `cc`-based reference build and
the harness's process handling, sandboxing, and newline discipline — is all written and all
lives in this stage; the remaining stages are dependency-free Rust that does not touch the
platform. CI becomes worth its upkeep once the scoreboard leaves 0 and there is a baseline to
regress against.

The scoreboard is identical on both platforms, as it must be — it is `0/31` with an empty
`ruta` regardless of what the reference does.

Under the reference interpreter the two platforms diverge sharply, which is worth knowing
before reading a `files.lua` or `main.lua` result:

| File                   | Windows                        | Linux (WSL2, Arch)                      |
| ---------------------- | ------------------------------ | --------------------------------------- |
| `all.lua` (`_port`)    | stops at `files.lua:467`       | **`final OK !!!`, exit 0**, 387/2 lines |
| `files.lua` (`_port`)  | stops at line 467, `/dev/null` | **exit 0**, 12 lines                    |
| `main.lua`             | stops at line 41               | stops at line 207, `assertion failed!`  |
| `attrib.lua` (`_port`) | exit 0                         | exit 0, 7 lines                         |

**The full suite passes on Linux.** `/dev/null` exists, so the unguarded flush block in
`files.lua` is not a problem there.

`main.lua` fails on both platforms but for unrelated reasons. On Windows it is the Unix shell
assumption at line 41. On Linux it reaches line 207, which asserts on
`warning: unable to load readline library 'xuxu'` — a message only a Lua built against
readline produces. The reference is deliberately built with `LUA_USE_POSIX` rather than
`LUA_USE_LINUX` precisely to avoid the readline dependency, so this failure is a consequence
of a build choice rather than a platform limit.

None of this changes what the scoreboard means. A file that fails identically on both sides
still counts as a match, so `main.lua` remains a useful differential case on both platforms —
just with a different amount of the file being compared.

## Where the code lives

| Path                                      | Role                                               |
| ----------------------------------------- | -------------------------------------------------- |
| `crates/ruta-conformance/src/manifest.rs` | manifest schema, loading, validation               |
| `crates/ruta-conformance/src/outcome.rs`  | what a run produced, and when two runs agree       |
| `crates/ruta-conformance/src/sandbox.rs`  | throwaway directories, process execution, timeouts |
| `crates/ruta-conformance/src/run.rs`      | the harness: what to feed both sides               |
| `crates/ruta-cli/tests/conformance.rs`    | the conformance scoreboard                         |
| `crates/ruta-cli/tests/parse.rs`          | the parse scoreboard                               |
| `crates/ruta-cli/tests/common/mod.rs`     | the little both scoreboards share                  |

The integration test lives in `ruta-cli` rather than in `ruta-conformance` because
**`cargo test` does not build another package's binary.** With the test in
`ruta-conformance`, a rebuilt `ruta` would not be the one being scored — the scoreboard would
silently grade a stale binary. Keeping the test in the same package as the `[[bin]]` makes
cargo build it and pass the path through `CARGO_BIN_EXE_ruta`.

The manifest is validated at load time rather than lazily: exactly one of `tier` and `skip`
must be present on each entry, and the entry list is cross-checked against
`vendor/lua-tests/*.lua`. A file missing from the manifest would otherwise be silently
excluded from the denominator.

`differential` takes a script as a **string**, not a path:

```rust
pub fn differential(&self, script: &str) -> Result<Comparison>
```

Stage 5 adds differential fuzzing over generated arithmetic expressions, which needs to feed
in generated source directly. A path-based signature would not be reusable there.

The parse scoreboard uses a second entry point, `parse_file`, which takes a path instead.
Two reasons: `strings.lua` is not valid UTF-8, so it cannot become a `&str` at all, and
copying the bytes into both sandboxes under one name gives both sides the same chunk name —
without which every error case would differ on its path alone.
