# conformance/

The scoring apparatus: what gets scored and the inputs the two scoreboards read. Nothing
here is build output — it is all committed on purpose.

| Path | Role |
| --- | --- |
| `manifest.toml` | Per-file tier and run options for `vendor/lua-tests/`. The conformance scoreboard's only input. |
| `extract-loads.lua` | Prelude that intercepts `load` while the suite runs, used to build the parse corpus. |
| `parse-corpus/` | The strings the suite passes to `load`, one file each. The parse scoreboard's second axis. |

The intermediate files extraction produces — the raw captures and the throwaway copy of the
suite it runs in — live under `target/` and are not committed.

## `parse-corpus/`

Regenerate with:

```bash
cargo xtask extract-parse-corpus
```

That runs each scored file under the reference interpreter with `extract-loads.lua` in front
of it, dumps every string chunk passed to `load`, and keeps the ones worth scoring: at most
4 KB, at most 100 per source file, no bytecode (`all.lua` round-trips every file through
`string.dump`), and no duplicates. `all.lua` itself is not extracted from — it re-runs the
whole suite, so everything it captures belongs to another file. The reasoning behind each
limit is in [docs/testing.md](../docs/testing.md#the-parse-scoreboard).

**The result is committed rather than generated on demand**, for two reasons.

It is not reproducible across platforms. `files.lua` stops at `/dev/null` on Windows and runs
to the end on Linux; `main.lua` stops at line 41 on Windows and at line 207 on Linux. Each of
those differences changes how many `load` calls the run reaches, so extracting on another
platform yields a different corpus — and a scoreboard denominator that moves with the machine
cannot be compared against another machine's.

It also carries judgment. The limits above decide what the parse scoreboard measures. Kept in
the repository, changing one shows up as a diff to review instead of a number that quietly
came to mean something else.

Regenerating on the same platform is byte-for-byte stable, so a rerun that produces a diff
means something really changed: the vendored suite, the reference build, or the limits.

The contents are excerpts of the official Lua test suite and carry its copyright — see
[NOTICE](../NOTICE).
