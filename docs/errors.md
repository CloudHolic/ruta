# Compile-time errors

Every message PUC-Lua 5.5.1 can raise while turning source text into a prototype, copied from
the C sources rather than paraphrased. The official test suite asserts on these strings and
on the line numbers that accompany them, so ruta has to reproduce them exactly — see
[INVARIANTS.md](../INVARIANTS.md) constraint 12.

The `Repro` column holds a snippet that produces the message; every one of them was observed
against the reference interpreter rather than deduced from the source. `-` means the message
needs a generated source too large to be worth keeping. The `Stage` column is the stage of
[roadmap.md](roadmap.md) at which ruta can first emit the message: stage 1 needs nothing but
the parser, stage 2 needs scope resolution, and stage 4 needs the code generator.

Runtime errors are not here. They arrive with the VM in stage 4 and get their own section
then. Errors from loading precompiled chunks (`lundump.c`) belong to stage 10.

## How a message is assembled

```
msg
 -> luaG_addinfo(L, msg, source, line)     ldebug.c:826   "%s:%d: %s"   source via luaO_chunkid
 -> token ? "%s near %s" : msg             llex.c:119     token text via txtToken
 -> luaD_throw(L, LUA_ERRSYNTAX)           llex.c:120
```

Two things vary along that path.

**Whether `near` appears.** `lexerror` takes the offending token as an argument and omits the
suffix when it is 0. `luaK_semerror` (`lcode.c:43-50`) always clears it, so no semantic error
ever carries a `near` clause.

**Which line is reported.** The default is `ls->linenumber`, the line the lexer is currently
on. `luaK_semerror` overrides it with `ls->lastline`, the line of the last consumed token —
which is why `global none` on line 1 followed by `return x` on line 2 reports line 2. Two
messages also carry a _second_ line inside their text: `check_match` names the line of the
token being closed, and `read_long_string` names the line the bracket opened on. That inner
line differs from the prefix line, and both are asserted by the suite. `lastline` is the line the last consumed token _ended_ on. A lookahead moves `linenumber` past
the token after the current one, so it can in principle run ahead of that; it never does in
practice, because every semantic error the parser raises fires after a token it consumed
itself — the three attribute errors all fire once `'>'` is in. `check_match` picks its longer
form by comparing the opening line with the line the error is reported on, not with the line
the opening token closed.

Syntax errors never pass through a message handler — `lexerror` throws `LUA_ERRSYNTAX`
directly. The one exception is described under [Parser recursion depth](#parser-recursion-depth).

## Token spelling

`luaX_token2str` (`llex.c:87-102`) has four cases:

| Token                              | Rendering                                                               |
| ---------------------------------- | ----------------------------------------------------------------------- |
| single byte, printable             | `'%c'`                                                                  |
| single byte, control character     | `'<\%d>'`                                                               |
| reserved word or multi-byte symbol | `'%s'` — `'end'`, `'..'`, `'::'`                                        |
| `TK_EOS` and above                 | bare, unquoted — `<eof>`, `<number>`, `<integer>`, `<name>`, `<string>` |

`txtToken` (`llex.c:104-113`) wraps it: for `TK_NAME`, `TK_STRING`, `TK_FLT`, and `TK_INT` it
ignores the token kind and quotes **the lexer's buffer** — the characters actually read so
far. This is why a bad escape reports the partial string including its opening quote:

```
x = "\xg"     ->  t:1: hexadecimal digit expected near '"\xg'
x = "\u{41"   ->  t:1: missing '}' near '"\u{41"'
```

The buffer is not the token, and it is not the line either. An implementation that reports the
whole line, or the token kind, matches neither.

Two rules follow from the buffer being what was read rather than what was written. A short
string shows its delimiters around the **decoded** value, because the escapes were resolved as
they were read — `x = 1 "a\65\u{42}"` reports `near '"aAB"'`. A long bracket shows its own
text, minus the newline that follows the opening bracket, which `read_long_string` consumes
without saving:

```
local x = 1
[[
abc]] -> t:3: unexpected symbol near '[[abc]]'
```

A token that is a single NUL byte carries **no `near` clause at all**. `lexerror` uses the
token value 0 to mean "no token to name", and a NUL byte token has exactly that value, so the
two collide: `\0=1` reports a bare `unexpected symbol`.

## Build options that change the grammar

Only one of the three `LUA_COMPAT_*` options affects the lexer or the parser.

| Option                   | Defined                            | Default | Affects the grammar        |
| ------------------------ | ---------------------------------- | ------- | -------------------------- |
| `LUA_COMPAT_GLOBAL`      | `luaconf.h:342-345`                | **on**  | yes                        |
| `LUA_COMPAT_MATHLIB`     | `luaconf.h:350-355`, commented out | off     | no — `lmathlib.c`, stage 9 |
| `LUA_COMPAT_APIINTCASTS` | not defined in `luaconf.h`         | off     | no — `lauxlib.h:252`, v2.0 |

`LUA_COMPAT_GLOBAL` guards exactly two blocks:

- **`llex.c:191-195`.** `"global"` stays in `luaX_tokens[]` at the `TK_GLOBAL` slot, but
  `luaX_setinput` clears its reserved-word marker (`ts->extra = 0`) and keeps the string in
  `ls->glbn`. The keyword is disabled at lexer-initialization time, not at build time.
- **`lparser.c:2117-2124`.** `statement` intercepts a `TK_NAME` equal to `ls->glbn`, calls
  `luaX_lookahead`, and treats it as a global declaration when the next token is one of
  `'<'`, `TK_NAME`, `'*'`, or `TK_FUNCTION`. Anything else falls through to an ordinary
  expression statement, so `global = 1; return global` compiles and returns 1.

That lookahead set is the whole disambiguation rule, and it is what ruta's `compat_global`
flag mirrors. It is on for all of v1.0. A reference built with `ltests.h` sets the option to 0
and rejects `global` as an identifier, which is what `goto.lua:328` is testing when it checks
for `T`.

## Lexical errors

All raised through `lexerror` (`llex.c:116`), all reported at the current line. "buffer" in
the `near` column means `txtToken` prints the partial token read so far, as described above.

| Message                                         | Raised at                       | `near`  | Repro                 | Stage |
| ----------------------------------------------- | ------------------------------- | ------- | --------------------- | ----- |
| `lexical element too long`                      | `llex.c:67` `save`              | -       | - (`heavy.lua:75`)    | 1     |
| `chunk has too many lines`                      | `llex.c:172` `inclinenumber`    | -       | -                     | 1     |
| `malformed number`                              | `llex.c:263` `read_numeral`     | buffer  | `x = 0x`              | 1     |
| `unfinished long string (starting at line %d)`  | `llex.c:307` `read_long_string` | `<eof>` | `x = [[`              | 1     |
| `unfinished long comment (starting at line %d)` | `llex.c:307` `read_long_string` | `<eof>` | `--[[ x`              | 1     |
| `unfinished string`                             | `llex.c:409` `read_string`      | `<eof>` | `x = "a`              | 1     |
| `unfinished string`                             | `llex.c:413` `read_string`      | buffer  | `x = "a` + newline    | 1     |
| `invalid long string delimiter`                 | `llex.c:505` `llex`             | buffer  | `x = [=x`             | 1     |
| `hexadecimal digit expected`                    | `llex.c:347` `gethexa`          | buffer  | `x = "\xg"`           | 1     |
| `missing '{'`                                   | `llex.c:369` `readutf8esc`      | buffer  | `x = "\u42"`          | 1     |
| `UTF-8 value too large`                         | `llex.c:373` `readutf8esc`      | buffer  | `x = "\u{7FFFFFFFF}"` | 1     |
| `missing '}'`                                   | `llex.c:376` `readutf8esc`      | buffer  | `x = "\u{41"`         | 1     |
| `decimal escape too large`                      | `llex.c:398` `readdecesc`       | buffer  | `x = "\300"`          | 1     |
| `invalid escape sequence`                       | `llex.c:443` `read_string`      | buffer  | `x = "\q"`            | 1     |

The last six reach `lexerror` through `esccheck` (`llex.c:336-342`), which passes `TK_STRING`.
`esccheck` first appends the offending character to the buffer, so it shows up in `near`.

Two observed pairs are worth keeping side by side, because the text is identical and only the
`near` clause distinguishes them:

```
x = "a          ->  t:1: unfinished string near <eof>
x = "a<newline> ->  t:1: unfinished string near '"a'
```

## Syntax errors

All raised through `luaX_syntaxerror` (`llex.c:124`), which reports the current token and the
current line.

| Message                                      | Raised at                                              | Repro                         | Stage |
| -------------------------------------------- | ------------------------------------------------------ | ----------------------------- | ----- |
| `%s expected`                                | `lparser.c:69` `error_expected`                        | `do`                          | 1     |
| `%s expected (to close %s at line %d)`       | `lparser.c:135` `check_match`                          | `do` + two blank lines        | 1     |
| `<name> or '...' expected`                   | `lparser.c:1090` `parlist`                             | `function f(1) end`           | 1     |
| `function arguments expected`                | `lparser.c:1168` `funcargs`                            | `x = a:b`                     | 1     |
| `unexpected symbol`                          | `lparser.c:1214` `primaryexp`                          | `x = )`                       | 1     |
| `cannot use '...' outside a vararg function` | `lparser.c:1289` `simpleexp`                           | `function f() return ... end` | 1     |
| `syntax error`                               | `lparser.c:1503` `restassign`                          | `a, f() = 1`                  | 1     |
| `syntax error`                               | `lparser.c:2013` `exprstat`                            | `x + 1`                       | 1     |
| `break outside loop`                         | `lparser.c:1556` `breakstat`                           | `break`                       | 1     |
| `'=' or 'in' expected`                       | `lparser.c:1748` `forstat`                             | `for i do end`                | 1     |
| `too many %s (limit is %d) in %s`            | `lparser.c:81` `errorlimit`                            | 201 `local` declarations      | 4     |
| `control structure too long`                 | `lparser.c:1654` `fixforjump`, `lcode.c:173` `fixjump` | - (`heavy.lua:51`)            | 4     |

`error_expected` formats `"%s expected"` with the output of `luaX_token2str`, which has
already added the quotes — the rendered message is `'end' expected`, not `''end'' expected`.
`check_match` uses the long form only when the opening token is on a different line; on the
same line it delegates to `error_expected`.

`break outside loop` needs only the parser's own block nesting, not name resolution, so it is
a stage 1 message despite looking like a scope check.

### The limit family

`errorlimit` (`lparser.c:74-84`) builds one message from three parts: the `what` string passed
by the caller, the numeric limit, and a `where` that is either `main function` or
`function at line %d`.

| `what`                   | Checked at       | Limit              |
| ------------------------ | ---------------- | ------------------ |
| `local variables`        | `lparser.c:337`  | `MAXVARS` (200)    |
| `upvalues`               | `lparser.c:373`  | `MAXUPVAL` (255)   |
| `items in a constructor` | `lparser.c:1050` | `MAX_CNST`         |
| `returns`                | `lcode.c:215`    | `MAXARG_B`         |
| `registers`              | `lcode.c:479`    | `MAX_FSTACK` (255) |
| `multiple results`       | `lcode.c:759`    | `MAXARG_C`         |

Both `where` variants, observed:

```
t:202: too many local variables (limit is 200) in main function near 'local'
t:205: too many local variables (limit is 200) in function at line 3 near 'local'
```

The suite reaches three of them: `errors.lua:743` (`registers`, via a call with 260
arguments), `errors.lua:748` (`upvalues`, via nested functions with 127 locals each), and
`errors.lua:771` (`local variables`). The other three are hard to trigger — `registers` fires
first on most sources that would otherwise exceed them.

## Semantic errors raised by the parser

All raised through `luaK_semerror` (`lcode.c:43-50`), which means **no `near` clause** and a
line number taken from `ls->lastline`.

| Message                                             | Raised at                             | Repro                                   | Stage |
| --------------------------------------------------- | ------------------------------------- | --------------------------------------- | ----- |
| `unknown attribute '%s'`                            | `lparser.c:1807` `getvarattribute`    | `local x <bogus> = 1`                   | 1     |
| `multiple to-be-closed variables in local list`     | `lparser.c:1838` `localstat`          | `local x <close>, y <close> = nil, nil` | 1     |
| `global variables cannot be to-be-closed`           | `lparser.c:1869` `getglobalattribute` | `global X<close>`                       | 1     |
| `attempt to assign to const variable '%s'`          | `lparser.c:320` `check_readonly`      | `local x <const> = 1; x = 2`            | 2     |
| `%s is global when accessing variable '%s'`         | `lparser.c:508` `buildglobal`         | `global _ENV, a; a = 10`                | 2     |
| `variable '%s' not declared`                        | `lparser.c:528` `buildvar`            | `global none` + `return x`              | 2     |
| `<goto %s> at line %d jumps into the scope of '%s'` | `lparser.c:583` `jumpscopeerror`      | `goto l; local x; ::l:: return x`       | 2     |
| `no visible label '%s' for <goto> at line %d`       | `lparser.c:740` `undefgoto`           | `goto nowhere`                          | 2     |
| `label '%s' already defined on line %d`             | `lparser.c:1571` `checkrepeated`      | `::a:: ::a::`                           | 2     |

The first `%s` of `%s is global when accessing variable '%s'` is `LUA_ENV`, so the rendered
message begins `_ENV is global`.

The first three need no scope information at all — they are decided from the attribute name
and the declaration being parsed — which settles a question left open in the stage 1 spec:
`global X<close>` is rejected during parsing, and ruta must reject it at stage 1 even though
the message travels the semantic path.

## Parser recursion depth

Deeply nested expressions do not produce a syntax error. `enterlevel` (`lparser.c:570`, used
at lines 1380, 1511, 1909, and 2054) calls `luaE_incCstack`, which raises a **runtime** error
through `luaG_runerror`:

```
return ((((...300 deep...))))   ->  C stack overflow
```

Two consequences follow, both observed:

- **There is no `chunk:line:` prefix.** `luaG_runerror` only prepends position information
  when the running frame is a Lua function; during parsing it is a C frame.
- **A message handler can still run.** `luaG_runerror` goes through `luaG_errormsg`
  (`ldebug.c:840-854`), which calls `L->errfunc` if one is installed — and `luaD_pcall` does
  not clear it for the protected parse. Under the standalone interpreter, the string `load`
  returns therefore has a traceback appended to it. Under `luac`, which installs no handler,
  it does not.

ruta needs a parser depth limit for the same reason, and the limit has to surface as a
catchable error rather than a stack overflow of the host. The depth itself need not match.

## What ruta cannot reproduce in stage 1

Four groups, recorded so their absence from the parse scoreboard is deliberate rather than
unnoticed.

**Chunk name formatting.** Everything above shows a `chunk:line:` prefix produced by
`luaO_chunkid`. The parse scoreboard compares files, so the chunk name is always a path;
the `[string "..."]` form that `load` produces for string chunks, and its truncation rules,
are not exercised until stage 4.

**Everything marked stage 4.** PUC-Lua is a single-pass compiler, so limits on registers,
constants, jumps, and local slots are raised by what is nominally the parser. ruta separates
code generation, so these cannot be raised until stage 4 no matter how complete the parser is.

**Runtime errors that look like compile errors.** `global 'print' already defined` is the
clearest example: it reads like a declaration check but is raised when the chunk runs.

**Chunks a file cannot carry.** The suite hands these cases to `load` as strings, and the
scoreboard writes them to files. A chunk containing `0x1A` does not survive the trip on
Windows, where the C runtime opens source files in text mode and stops there: `luac -p` reads
a truncated chunk while ruta reads the whole thing, and ruta is the one that matches `load`.

```
load("a" .. string.char(26) .. "1 = 1") -> syntax error near '<\26>'
luac -p, same bytes in a file -> syntax error near <eof>
```

`cargo xtask extract-parse-corpus` drops such chunks rather than grading them.
