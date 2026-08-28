# Invariants

Design constraints that cannot be reversed later. Everything else in `ruta` is open to
revision; these fourteen are not.

A constraint earns a place here when getting it wrong means rewriting a subsystem rather
than editing it. That is a narrow bar on purpose — this file stays short so that it keeps
being read.

Each item names the stages that are bound by it. Re-read the relevant items **before**
entering a stage, not while debugging it. Stage numbers refer to [docs/roadmap.md](docs/roadmap.md);
the reasoning behind the broader decisions lives in [docs/decisions/](docs/decisions/).

## Memory and object layout

**1. The garbage collector is non-moving.** Stages 3, 8.
The v2.0 C ABI layer exposes `lua_topointer` and raw userdata pointers. Pointers held by
C code cannot be invalidated, so objects cannot move. Moving and compacting collectors are
out of scope permanently. See [ADR 0003](docs/decisions/0003-non-moving-gc.md).

**2. Strings are contiguous bytes with a trailing NUL at a fixed address.** Stage 3.
No ropes, no small-string optimization that moves the payload. Lua strings can contain
embedded NULs, so the length is stored separately — the trailing NUL exists for the C ABI's
benefit, not as a terminator ruta itself relies on.

**3. The value representation reserves a userdata object variant and pinned memory.** Stage 3.
Full userdata is a C ABI concept, but leaving room for it is a layout decision. Adding a
variant to the value type after the heap is built touches every match arm in the runtime.

**4. Heap access goes through handles (indices) from the very first commit.** Stages 3, 8.
The collector itself can wait. Passing object references around directly cannot: introducing
a GC later would mean rewriting every site that holds a reference. Handles make the
`unsafe_code = "deny"` workspace lint affordable.

**5. All heap writes pass through a single entry point.** Stages 3, 8.
A generational collector's write barrier has to intercept every heap write. If write sites
are scattered, they all have to be found again later — and the ones that are missed are
silent corruption, not compile errors.

**6. No NaN boxing.** Stage 3.
It conflicts with constraint 1 and with the handle discipline of constraint 4, and it trades
clarity for a density this project does not need.

## Execution

**7. Frame register windows are contiguous slices.** Stage 4.
Registers must be addressable by integer stack index. The C ABI's stack indexing model and
the VM's own operand decoding both assume it.

**8. Lua function calls are not Rust recursion.** Stages 4, 7, 13.
Call frames live on an explicit stack. A coroutine must be able to yield from an arbitrary
call depth, which is impossible if the Lua call stack is interleaved with the host's. Native
function frames go on the same explicit stack, and so do the machine-code frames of stage 13
— see constraint 14.

**9. Error propagation has a single path.** Stages 6, 11.
v2.0 has to be able to swap in a `longjmp` bridge for the C ABI. That is a local change only
if there is one place to change.

## Observability

**10. Function prototypes reserve a slot for a debug information table.** Stages 4, 10.
Instruction-to-source-line mapping, local variable names with their active ranges, and
upvalue names. The whole `debug` library is in scope for v1.0, and this table cannot be
reconstructed after the fact.

**11. The dispatch loop reserves hook check points (count and line).** Stages 4, 10.
Retrofitting hook checks means touching the hot loop again once its shape has settled.

**12. Name and source-position provenance is attached from the AST node onward.** Stage 1.
Lua error messages name the kind and identity of a variable — `attempt to index a nil value
(local 'x')` — and the official test suite asserts on those exact strings. Provenance that
is not carried from the parser cannot be recovered in the VM. `goto.lua` goes further and
asserts on the *line number* inside a compile error (`checkerr(..., "%:2%:")`), so positions
have to survive the IR as well as names.

## Structure

**13. `ruta-compile` does not depend on `ruta-runtime`.** Stage 4.
The IR is the backend boundary, and a crate boundary is the only way to make that structural
rather than aspirational — a compiler that cannot name a runtime type cannot accidentally
reach into one.

The consequence to plan for: **code generation cannot build heap objects**, so it cannot
intern strings. The constant pool holds unresolved constants — plain `String`, `i64`, `f64` —
and the runtime interns them when a chunk is loaded. This is what PUC-Lua's undump does, and
stage 10 needs it again for `string.dump` round-trips.

Both crates are split out of `ruta-syntax` and `ruta-runtime` at stage 4. The boundary has to
exist from the moment they do; retrofitting it means untangling every site where the
compiler learned to depend on runtime representation.

**14. The IR and the collector are written as if a second backend already existed.** Stages 4, 8, 13.
v3.0 lowers the same IR to machine code, and two things about that cannot be added afterward.

Native frames share the explicit call stack of constraint 8. Machine code that used the host
stack instead would reintroduce exactly the problem constraint 8 exists to prevent: a
coroutine cannot yield across a frame the host owns. The generated code therefore has to
maintain ruta's frame stack rather than rely on the hardware's, and an IR designed without
that in mind produces code that cannot.

Root discovery is not expressed in terms of VM frames. The collector walks the frame stack
asking each frame for its roots; a frame answers for itself. Precise scanning of a native
frame needs a root map the code generator emits, and a collector written to read VM register
windows directly has no place to put one. Stage 8 builds the collector nine stages before
stage 13 needs this, which is the whole reason it is written down here.

Neither reservation costs anything at stage 4. Both are a rewrite at stage 13.
