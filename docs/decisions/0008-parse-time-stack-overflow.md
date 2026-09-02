# 8. Parse-time stack overflow is a compile error with a position

Status: accepted (2026-09-01)

## Context

The reference does not treat deep nesting as a syntax error. Its parser counts nesting against
a fixed budget - `LUAI_MAXCCALLS`, 200, of which three are spent before the parser starts - and
raises a _runtime_ error when it runs out. Two things follow from that, both observed:

`luac: C stack overflow`

There is no `chunk:line:` prefix, because position information is prepended only when the frame
that raised is a Lua function, and during a parse it is a C frame. And because the protected
parse leaves the message handler installed, the string `load` returns carries a traceback that
a syntax error's does not.

Measured by bisection, the ceiling lands between 196 and 198 source levels depending on the
construct, and at 98 for nested function bodies, which cost two levels each.

`ruta` cannot reproduce the message. `C stack overflow` names a stack it does not have, which
the rule in `AGENTS.md` covers: a phrase that is not true here is not copied.

Nor could `ruta` reproduce the behavior. Its parse ran on whatever stack the platform hands the
main thread - 1 MiB on Windows - and the debug build died at 134 levels, below the reference's
own ceiling. A counter alone would have been unreachable: the effective limit was set by frame
sizes and the optimization level, and moved whenever the parser was edited.

## Decision

The parse runs on a thread whose stack size `ruta-cli` chooses, and the parser refuses nesting
past a limit of its own. Past that limit it reports:

`ruta: deep.lua:1: stack overflow`

Two departures from the reference, both deliberate. The message drops `C`. And the refusal
carries a position, through the same channel as every other compile-time refusal, because the
reference's lack of one is an artifact of C frames rather than a rule about the language.

The limit is 1000, counted at two points - entering an expression and entering a block. It sits
above the reference's 196-198, so **every chunk the reference accepts is accepted here**, and
far below what the thread's stack supports.

## Consequences

Text and shape both differ from the reference. The suite never reaches this path - its deepest
nesting is 12 levels, the extracted corpus 3 - so no scoring is lost, and no scoreboard guards
the behavior either; a probe does.

Because the limit is higher than the reference's, a divergence can only run one way: `ruta`
accepting a chunk the reference refuses. The reverse would be a bug.

This settles the parse-time case only. The four places the suite asserts on `C stack overflow`
(`errors.lua:397`, `errors.lua:640`, `cstack.lua:108`, `cstack.lua:141`) are runtime recursion,
they are asserted, and stage 4 decides them separately - the two limits are unrelated values.
