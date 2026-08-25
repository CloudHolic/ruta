# 6. Diagnostics end their lines the way the platform's C runtime does

Status: accepted (2026-08-25)

## Context

On Windows the reference interpreter writes `\r\n` where its source writes `\n`. Its standard
streams are C `FILE*` handles in text mode, and the C runtime translates on the way out.

```
lua.exe -e "print('a')"          ->  61 0D 0A
lua.exe -e "io.stderr:write..."  ->  62 0D 0A
```

`ruta` writes bytes, so it emitted `\n`. The parse scoreboard compares stdout, stderr and the
exit code byte for byte, which made every case a mismatch on Windows however correct the
message was:

```
reference: ... near '"abc\x"'  \r \n
ruta:      ... near '"abc\x"'  \n
```

Nothing else differed. The scoreboard would have stayed at `0/637` through stages 1 to 3.

This went unnoticed in stage 0 because the sanity check runs the reference on both sides, and
CRLF matches CRLF.

Text mode is not an artifact of how the reference happens to be built. It is observable Lua
behavior: `files.lua` opens the same path with `"w"` and with `"wb"` and checks that the two
disagree, so the io library has to model it regardless.

## Decision

`ruta` ends a diagnostic line with `\r\n` on Windows and `\n` elsewhere.

Stage 1 applies this to stderr in `ruta-cli`, which is all the output that exists. The same
rule covers stdout when the VM arrives in stage 4 and the io library in stage 6; the mechanism
belongs wherever the stream is written, not in `ruta-syntax`, which produces message bytes and
no line breaks.

Rejected: normalizing CRLF on both sides inside the harness. It is a smaller change today and
a worse one from stage 4 on, because it would also mask a genuine newline difference in the
output of a Lua program under test - which is exactly what `files.lua` is looking for.

## Consequences

A message assembled in `ruta-syntax` carries no terminator. The writer adds it. Any future
code that writes a diagnostic has to go through that writer rather than `println!` or
`eprintln!`, whose `\n` is unconditional.

The rule is per-platform, so the conformance and parse scoreboards can only be compared
against a reference built for the same platform. That was already true.
