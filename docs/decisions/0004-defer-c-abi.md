# 4. Defer the C ABI layer to v2.0

Status: accepted (2026-08-24)

## Context

Lua's C API is part of the language as it is actually used, so a complete reimplementation
has to provide it eventually. The question is when.

Building it first means the value representation, the stack model, and error propagation are
all shaped by C's requirements from the start — while none of them has been exercised by a
working interpreter yet. Guessing at those requirements before the runtime exists produces a
design that fits neither side well.

## Decision

Finish the specification and the standard library in pure Rust first (v1.0). Add the C ABI
embedding layer on top as an adapter afterward (v2.0, stage 11).

What is deferred is the implementation, not the constraints. The parts that cannot be
retrofitted are fixed now, in [INVARIANTS.md](../../INVARIANTS.md): the non-moving
collector (1), string representation (2), the userdata variant (3), contiguous register
windows (7), and a single error propagation path (9).

## Consequences

The split falls out naturally in the test suite. Files that depend on the `T` library from
`ltests.c` cannot mean anything before the C API exists, so they sit in the `v2.0` tier of
`conformance/manifest.toml` — currently `api.lua`, `code.lua`, and `memerr.lua`. This is not
a coincidence to be worked around; it is the same boundary seen from the scoreboard.

The v1.0 scoreboard therefore has a ceiling below 100% of the suite, and that is the intended
reading of it. See [testing.md](../testing.md).

The risk accepted is that stage 11 discovers a constraint that was not anticipated here.
The mitigation is that the five items above cover the ones with heap-wide or loop-wide
consequences; anything else should be adapter-local.
