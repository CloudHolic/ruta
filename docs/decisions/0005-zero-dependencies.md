# 5. `ruta-syntax` and `ruta-runtime` take no dependencies

Status: accepted (2026-08-24)

## Context

Every single thing this project exists to build by hand is available as a mature crate. Hash
maps, arenas and generational indices, parser combinators and lexer generators, shortest
round-trip float formatting — all of it is one line in a manifest away.

That is the whole problem. A policy left implicit is a policy abandoned under deadline
pressure, and the first time a table implementation gets swapped for `hashbrown` the project
stops being the thing it was for.

## Decision

The policy is enforced at crate boundaries rather than by intent.

| Crate | Dependencies |
|---|---|
| `ruta-syntax` | none |
| `ruta-runtime` | none, except `ruta-syntax` |
| `ruta-cli` | minimal (argument parsing) |
| `ruta-conformance`, `xtask` | unrestricted |

Specifically prohibited in the two core crates: hash map crates (`hashbrown`, `indexmap`) and
`std::collections::HashMap` as the Lua table implementation, which is an array part and a
hash part in one object; GC and arena crates (`gc-arena`, `slotmap`, `generational-arena`);
parser and lexer generators (`nom`, `chumsky`, `logos`, `pest`).

Float formatting needs no crate at all: Rust's `{}` already produces the shortest
round-tripping representation, which is what Lua 5.5 requires.

## Consequences

Tooling convenience is still available where it costs nothing — `ruta-conformance` and
`xtask` use `anyhow`, `serde`, `toml`, and `cc` freely. Confining them to those crates is
what makes the restriction on the other two sustainable rather than merely painful.

When the policy starts to feel like it should bend, the process is to write an ADR in this
directory first and decide afterward. Bending it silently is the failure mode this decision
exists to prevent, so the table above is the authoritative statement of it — not a summary of
a rule kept somewhere else.
