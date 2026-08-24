# 1. Rust as the host language

Status: accepted (2026-08-24)

## Context

The point of `ruta` is to build a bytecode VM, a garbage collector, and a code generator by
hand. That rules out any host language that hides memory layout or allocation behind a
runtime, because the parts being hidden are exactly the parts worth building.

There is a second requirement that pulls in the same direction. v2.0 exposes a C ABI
embedding layer, so the host language cannot carry a runtime of its own into the shared
library.

Against that, a project of this size needs a refactoring safety net. Reworking a value
representation or a call convention across a whole runtime is routine here, and doing it
without help from a type system is how learning projects stall.

## Decision

Rust.

`unsafe_code = "deny"` is set as a workspace-wide lint and relaxed per crate only where a
concrete need appears.

## Consequences

Designing the heap around handles (indices) rather than references is what makes the
`unsafe_code` denial affordable — see [INVARIANTS.md](../../INVARIANTS.md) constraint 4. That
constraint exists because of this decision, not independently of it.

Some of what a C implementation gets for free has to be paid for explicitly: tagged unions
become enums with exhaustive matches, and pointer-chasing patterns from PUC-Lua do not
translate directly. This is a cost accepted deliberately, since the compiler catching a
missed case is worth more here than a shorter port.
