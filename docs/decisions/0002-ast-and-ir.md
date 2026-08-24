# 2. Keep an AST and an IR

Status: accepted (2026-08-24)

## Context

PUC-Lua is a single-pass compiler: the parser emits bytecode as it goes, with no
intermediate tree. It is a genuinely good design for its goals — compilation is fast and
memory use is bounded — and copying it would be the shortest route to a working Lua.

But in that design, scope resolution, register allocation, and peephole optimization all
live inside the parser. Reading any one of them means reading all of them.

## Decision

`ruta` inserts two stages PUC-Lua does not have:

```
source -> lexer -> parser -> AST -> scope resolution -> IR -> register allocation -> bytecode -> VM
```

## Consequences

Each pass can be observed on its own, which is the reason the project exists. Scope
resolution becomes an independent pass over the AST instead of bookkeeping smeared through
parsing; register allocation is separable from code generation.

The staged roadmap depends on this split. Stages 1, 2, and 4 are only separable milestones
because there is a data structure to hand off between them.

The costs are real and accepted: more memory during compilation, an extra representation to
keep in sync, and a slower compile path than PUC-Lua's. None of them affect runtime
semantics, and the official test suite does not measure compile time.
