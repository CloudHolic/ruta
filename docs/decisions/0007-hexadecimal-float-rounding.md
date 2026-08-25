# 7. Hexadecimal float literals are rounded correctly, even where the reference is not

Status: accepted (2026-08-25)

## Context

Lua does not convert `0x1.8p3` itself. `lua_strx2number` is defined as the C library's `strtod`
whenever the library is C99, and it carries its own fallback only for C89 builds. So the value a
hexadecimal float literal takes is whatever the platform's `strtod` says.

On Windows that answer is not always the nearest double. Measured against an exact rounding
computed in 128-bit integers, over 4000 random hexadecimal float literals:

```
ruta differs from exact:        0
the reference differs from exact: 118
```

All 118 are one ULP low. They all have more than sixteen significant hexadecimal digits, which
is where the library appears to stop tracking what it discarded. glibc's `strtod` is correctly
rounded, so a reference built on Linux agrees with `ruta` on the same 118.

This matters because the scoreboards compare `ruta` against a reference built for the same
platform. A conformance case that printed one of those literals would count as a mismatch on
Windows and a match on Linux.

Two observations bound that risk.

- The official suite's longest hexadecimal float literal has 17 significant digits, and all six
  of its literals longer than four digits produce identical bits either way.
- `strings.lua` round-trips `string.format("%a", n)` through `tonumber`, which looks like the
  dangerous path and is not: a `%a` string names a double exactly, so no rounding happens on the
  way back.

## Decision

`ruta` converts hexadecimal float literals correctly - round to nearest, ties to even, subnormals
included - and does not reproduce the platform library's error.

The alternative, matching whatever `strtod` does, would mean being wrong on Linux to be equal on
Windows, and would make the answer depend on which C library the reference was built against.

## Consequences

The differential harness is no longer the whole truth for this one conversion. A hexadecimal
float mismatch on Windows is evidence about the reference, not about `ruta`, and the way to
settle it is the exact 128-bit rounding rather than either implementation.

If a conformance case ever does catch this, the fix is to record it - not to make the converter
worse. The same converter serves `tonumber` in stage 6, so the decision reaches beyond the lexer.
