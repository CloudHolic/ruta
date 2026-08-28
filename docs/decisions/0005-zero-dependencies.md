# 5. Dependencies are refused where they would build what ruta exists to build

Status: accepted (2026-08-24), restated (2026-08-26)

## Context

Every single thing this project exists to build by hand is available as a mature crate. Hash
maps, arenas and generational indices, parser combinators and lexer generators, shortest
round-trip float formatting — all of it is one line in a manifest away.

That is the whole problem. A policy left implicit is a policy abandoned under deadline
pressure, and the first time a table implementation gets swapped for `hashbrown` the project
stops being the thing it was for. The README's claim — a Lua whose VM, collector, and code
generator were built by hand — is either true or it is advertising.

The original form of this decision was a list of banned crates. That worked while the only
question was `hashbrown` versus a hand-written table, and stopped working as soon as v3.0
put a native backend on the roadmap: writing an instruction encoder, a relocation scheme, and
three object file formats is a different project, and what it teaches is assembler mechanics
rather than backend design. A list cannot say why that is different from `hashbrown`.

## Decision

The rule is a principle, and the crate table is its approximation.

> A dependency is refused when it would implement something `ruta` exists to have built by
> hand. It is allowed when it is scaffolding around that.

`ruta-syntax` and `ruta-runtime` take no dependencies because what has to be built by hand is
exactly what lives inside them. Zero is the consequence, not the goal.

| Crate | Dependencies | Why |
|---|---|---|
| `ruta-syntax` | none | the lexer and parser are the deliverable |
| `ruta-runtime` | none, except `ruta-syntax` | tables, the heap, the collector, the VM |
| `ruta-compile`, `ruta-bytecode` (stage 4) | none | the IR and code generation are the deliverable |
| `ruta-capi` (stage 11) | none | `extern "C"` and `#[repr(C)]` need no crate, and the C ABI *is* the exercise |
| `ruta-codegen` (stage 13) | a code generator backend | see below |
| `ruta-cli` | minimal | argument parsing |
| `ruta-conformance`, `xtask` | unrestricted | scaffolding by definition |

Three categories, applied in this order:

**Refused** — implements the deliverable. Hash map crates (`hashbrown`, `indexmap`) and
`std::collections::HashMap` as the Lua table, which is an array part and a hash part in one
object. GC and arena crates (`gc-arena`, `slotmap`, `generational-arena`). Parser and lexer
generators (`nom`, `chumsky`, `logos`, `pest`). Dynamic library loading (`libloading`) —
`dlopen` and `LoadLibrary` are fifty lines of `extern "C"`, and stage 11 exists to write
exactly that kind of code.

**Unnecessary** — convenience macros and thin wrappers. `thiserror`, `derive_more`,
`bytemuck`. These break no goal, which is why they need a separate category: the reason not
to take them is that they save almost nothing here and cost the zero. `thiserror` in
particular has less to offer than it looks. Syntax errors have to match PUC-Lua byte for
byte, with `near <token>` and chunk name truncation, so their `Display` is a hand-written
function either way; and a Lua runtime error is a *value* thrown to `pcall`, not a Rust error
type, so there is nothing to derive.

**Allowed** — neither of the above, and writing it by hand would be plainly unproductive.
Stage 13's backend is the first real member: what that stage is for is attaching a code
generator to the IR, defining a calling convention, and scanning roots precisely in native
frames, and every one of those survives using an existing backend. It lives in its own crate
so the boundary stays enforced by the compiler rather than by intent.

Float formatting needs no crate at all: Rust's `{}` already produces the shortest
round-tripping representation, which is what Lua 5.5 requires.

## Consequences

Zero dependencies in the core is worth more than the sum of what each crate would have saved.
It composes with `unsafe_code = "deny"` to make the whole runtime auditable; it removes
version churn from a project that will go months between sessions; and it is a genuinely
uncommon property for a Lua implementation — `mlua` and `rlua` are bindings, and `piccolo`
takes `gc-arena`.

The real cost is that the "unnecessary" category has to be held. It is the one that erodes:
`thiserror` breaks nothing, and neither does `smallvec`, and by `smallvec` the boundary is no
longer obvious. Part of what zero buys is not having to make that judgment call.

When the policy starts to feel like it should bend, the process is to write an ADR in this
directory first and decide afterward. Bending it silently is the failure mode this decision
exists to prevent.
