# 3. The collector is non-moving

Status: accepted (2026-08-24)

## Context

Moving collectors are attractive. Compaction removes fragmentation, bump allocation is
close to free, and copying collectors have a pleasing simplicity.

The v2.0 C ABI layer makes them impossible. `lua_topointer` hands out an address, and full
userdata hands out a pointer to a payload that C code writes through. Neither can be
invalidated behind the caller's back, and neither can be found again to be fixed up.

The timing matters more than the choice. This constraint has to be fixed before the value
representation is designed in stage 3, because reversing it later means rewriting the entire
heap.

## Decision

The collector is non-moving. Objects keep their address for their whole lifetime. Moving and
compacting collectors are permanently out of scope.

Stage 8 builds up in this order: mark-sweep, then weak tables, then ephemerons, then
finalizers, then incremental and generational modes. All of these are compatible with
non-moving collection.

## Consequences

Fragmentation has to be handled by allocation strategy rather than by compaction.

The generational mode still needs a write barrier, which is why every heap write goes
through one entry point — [INVARIANTS.md](../../INVARIANTS.md) constraint 5. That is a
consequence of choosing non-moving *and* wanting generational, not of either alone.

This decision is what makes [ADR 0004](0004-defer-c-abi.md) coherent: the C ABI
implementation is deferred, but the constraints it imposes are not.
