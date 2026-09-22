# 9. ruta writes the bytes a Lua program writes; the harness undoes the reference's C runtime

Status: accepted (2026-09-22)

Supersedes [0006](0006-windows-line-endings.md).

## Context

ADR 0006 made `ruta` end diagnostic lines with `\r\n` on Windows so that it would match the
reference byte for byte. The reference is a C program whose standard streams are text-mode
`FILE*` handles, and the C runtime puts a `\r` in front of every `\n` on the way out — not
only at the end of a line. `print("a\nb")` comes out as `a\r\nb\r\n`. Once the VM could run
programs that print line breaks inside strings, "end the line with `\r\n`" no longer
described what the reference does.

None of this is Lua. The manual says a `b` in an `io.open` mode "is needed in some systems to
open the file in binary mode" and asks nothing further; a Lua that writes `\n` on Windows is
still a Lua. ADR 0006 gave two reasons to match anyway. One was the scoreboard, which compares
against the reference byte for byte. The other was that `files.lua` opens a file with `"w"`
and with `"wb"` and checks that the two disagree. It does not: `"wb"` appears only where it
writes the output of `string.dump`, and the suite has to pass on Linux, where the two modes
are the same, so it cannot depend on them differing.

That leaves the scoreboard, which is a question for the harness. ADR 0006 rejected answering
it there because normalizing line endings on both sides would hide a real newline difference.
Undoing the translation on the reference's side alone hides nothing. Text mode adds a `\r`
before each `\n` and changes nothing else, so taking the `\r` off every `\r\n` the reference
wrote recovers exactly the bytes its Lua program wrote.

The C runtime decides one more thing on the reference's behalf: how a NaN is spelled. MSVC
writes `-nan(ind)` where glibc writes `-nan`.

## Decision

`ruta` writes the bytes a Lua program writes, on every platform.

On Windows the harness takes the reference's output back to what its program wrote before
comparing: every `\r\n` becomes `\n`, and `nan(ind)` and `nan(snan)` become `nan`. It does
this to whatever the reference binary produces, whichever seat it sits in, so the sanity check
with the reference on both sides still matches everywhere.

ADR 0007 is the same decision on the way in: `ruta` reads a hexadecimal float the way the
language means it, not the way the platform's `strtod` happens to.

## Consequences

The line-ending inversion is exact. The NaN one is a guess: a program that prints `nan(ind)`
itself would have it rewritten on the reference side. Nothing in the suite does.

When the io library arrives, `"w"` and `"wb"` behave the same, which the manual allows.

What `ruta` writes no longer depends on the platform, so a scoreboard on Windows and one on
Linux measure the same thing.
