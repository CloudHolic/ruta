# Bytecode

What `ruta` compiles a chunk to, and how the VM runs it.

**This is an internal representation tied to a version of `ruta`, not a compatibility
promise.** Any release may renumber the opcodes, change their operands or the layout of a
prototype. Nothing reads bytecode that another version of `ruta` wrote, and there is no
serialized form yet: `string.dump` (stage 10) will be the first. The opcode table below is
generated from the code, so it describes exactly the version it ships with.

## Execution model

`ruta` is a register machine. Every call has a **frame**: a window of registers on one value
stack shared by all frames, beginning at the frame's **base**. A function declares how many
registers it needs (`max_registers`, at most 255); an operand naming register `r` means the
slot at `base + r`.

A local variable keeps one register for the whole of its scope. Temporaries live above the
locals.

**Calls.** The function sits in register `callee` and its arguments in the registers right
after it. The callee's frame begins at `callee + 1`, so a call may overwrite every register
from `callee` up: whatever has to survive the call is kept below it. The results come back
starting at `callee`, trimmed or padded with nil to the number the call asked for.

**Counts that are not known in advance.** An argument, result or value count equal to `MULTI`
(255) means "as many as there are". An instruction that produces such a run — a call asking
for all results, or `...` spread — marks where it ends, and the instruction right after it
consumes the run: a call passing them all as arguments, a `return`, a table constructor's last
batch, or a tail call.

**Extra arguments.** A function declared with `...` keeps its extra arguments just below its
frame; its fixed parameters are moved up past them. `...t` does the same and also puts a table
of the extras in the register right after the parameters.

**Tail calls** replace the calling frame instead of growing the stack. A host function reached
by a tail call runs first and the calling frame then returns what it answered, so an error in
it still points at the tail call.

**Upvalues.** A closure shares a captured local with the frame that declared it through a cell
that points at the local's stack slot while the frame lives, and holds the value once the slot
goes away. `CloseUpvals` moves the value in at the end of a scope that leaves by a jump; a
`return` closes the frame's cells itself.

**Globals.** The outermost function of a chunk has exactly one upvalue, `_ENV`, which the loader
fills with the table of globals. A global is a field of it: `x` reads through `Index` and
`x = v` writes through `SetIndex`. `global x = v` uses `DefineGlobal`, which fails when `x` is
already defined.

**Counted `for`** uses four consecutive registers — the counter, the limit, the step, and the
copy the body sees. `ForPrep` validates and either enters the body or jumps past it; `ForLoop`
advances and jumps back while the loop runs. An integer loop counts its iterations up front, so
it cannot overflow at either end of the range; a float loop adds the step each time round.

## Encoding

Instructions are variable-length. **The first byte is the opcode, and it alone determines the
length**: every opcode has a fixed list of operands, and the table below gives their widths.
Operands follow in order, little-endian.

- A register or a count is one byte.
- A constant pool index is two bytes. `LoadConstWide` takes four, for pools that outgrow two.
- A jump offset is a signed four-byte displacement, counted from the first byte of the
  instruction after the jump. It is always four bytes, so an instruction's length never depends
  on how far it jumps.

A conditional branch only names one target; the other is the next instruction. A counted
`for` names the exit from `ForPrep` and the start of the body from `ForLoop`.

## Prototypes

A compiled function is a **prototype**:

| Field | Contents |
|---|---|
| `params` | How many fixed parameters it takes. |
| `vararg` | Whether it was declared with `...`, `...t`, or neither. |
| `max_registers` | The size of its frame. |
| `code` | Its instructions. |
| `constants` | Integers, floats and strings. Strings are bytes here; the loader turns each into a heap string, so equal constants share one. |
| `children` | The functions it creates, in the order `Closure` names them. |
| `upvals` | For each upvalue, its name and where it comes from: a register of the enclosing function, or one of the enclosing function's own upvalues. The outermost function's `_ENV` is written as the enclosing register 0 and supplied by the loader. |
| `source` | The chunk name: `@` before a file name, `=` before a name shown as it is. |
| `line_defined`, `last_line_defined` | Where the function's text starts and ends; 0 for the outermost function. |
| `lines` | `(pc, line)` pairs in `pc` order, one wherever the line changes. |
| `locals` | Each named local: its name, its register, and the `pc` range in which it is in scope, the start included and the end excluded. A declaration no path reaches is left out. |

A chunk is a tree of prototypes whose root is the outermost function. Once loaded, each
prototype becomes a heap object of its own, so a closure keeps alive only the prototype it
runs and the ones it can create.

## Opcodes

Operands are listed in the order they are written. `MULTI` is 255.

<!-- opcode table: generated by `cargo xtask bytecode-table` -->

| Byte | Mnemonic | Operands | Length | Meaning |
| ---: | --- | --- | ---: | --- |
| 0 | `LoadNil` | `dest` u8 | 2 |  |
| 1 | `LoadTrue` | `dest` u8 | 2 |  |
| 2 | `LoadFalse` | `dest` u8 | 2 |  |
| 3 | `LoadConst` | `dest` u8, `constant` u16 | 4 | `constant` indexes the prototype's own pool. |
| 4 | `LoadConstWide` | `dest` u8, `constant` u32 | 6 | `constant` indexes the prototype's own pool. |
| 5 | `Move` | `dest` u8, `src` u8 | 3 |  |
| 6 | `GetUpval` | `dest` u8, `index` u8 | 3 |  |
| 7 | `SetUpval` | `index` u8, `src` u8 | 3 |  |
| 8 | `CloseUpvals` | `from` u8 | 2 | Closes every upvalue pointing at `from` or above. |
| 9 | `Closure` | `dest` u8, `child` u32 | 6 | `child` indexes the enclosing prototype's own children. |
| 10 | `Vararg` | `first` u8, `count` u8 | 3 | Reads `...` into `first ..`. `count` is `MULTI` when the values run to the top of the frame. |
| 11 | `NewTable` | `dest` u8, `array_hint` u32, `hash_hint` u32 | 10 | `array_hint` and `hash_hint` are how many entries of each part to make room for. |
| 12 | `Index` | `dest` u8, `object` u8, `key` u8 | 4 |  |
| 13 | `SetIndex` | `object` u8, `key` u8, `src` u8 | 4 |  |
| 14 | `DefineGlobal` | `env` u8, `key` u8, `src` u8 | 4 | Unlike `SetIndex` this fails when the old value is not nil, with `global '%s' already defined`. `false` counts as defined; only nil passes. |
| 15 | `SetList` | `table` u8, `first` u8, `count` u8, `first_index` u32 | 8 | `table[first_index ..] = first .. first + count`. |
| 16 | `SetListSpread` | `table` u8, `first` u8, `first_index` u32 | 7 | The same, with the values running to the top of the frame. |
| 17 | `Neg` | `dest` u8, `operand` u8 | 3 |  |
| 18 | `Not` | `dest` u8, `operand` u8 | 3 |  |
| 19 | `Len` | `dest` u8, `operand` u8 | 3 |  |
| 20 | `BNot` | `dest` u8, `operand` u8 | 3 |  |
| 21 | `Add` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 22 | `Sub` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 23 | `Mul` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 24 | `Div` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 25 | `IDiv` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 26 | `Mod` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 27 | `Pow` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 28 | `Concat` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 29 | `Eq` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 30 | `Ne` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 31 | `Lt` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 32 | `Le` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 33 | `Gt` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 34 | `Ge` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 35 | `BAnd` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 36 | `BOr` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 37 | `BXor` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 38 | `Shl` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 39 | `Shr` | `dest` u8, `left` u8, `right` u8 | 4 |  |
| 40 | `Call` | `callee` u8, `args` u8, `results` u8 | 4 | The arguments sit at `callee + 1 ..`, the results land at `callee ..`. Either count is `MULTI`. |
| 41 | `TailCall` | `callee` u8, `args` u8 | 3 | Replaces the running frame. `args` is `MULTI` when they run to the top of the frame. |
| 42 | `Return` | `first` u8, `count` u8 | 3 | Hands back `first .. first + count`. `count` is `MULTI` when the values run to the top of the frame. The frame's open upvalues close here. |
| 43 | `Jump` | `offset` i32 | 5 | `offset` counts from the instruction after this one, as every jump's does. |
| 44 | `JumpIfTrue` | `cond` u8, `offset` i32 | 6 | Jumps when `cond` holds anything but nil or false. |
| 45 | `JumpIfFalse` | `cond` u8, `offset` i32 | 6 | Jumps when `cond` holds nil or false. |
| 46 | `ForPrep` | `control` u8, `offset` i32 | 6 | `control`, `control + 1`, `control + 2` and `control + 3` are the counter, the limit, the step and the copy the body sees. Jumps past the loop when the body does not run at all. |
| 47 | `ForLoop` | `control` u8, `offset` i32 | 6 | Steps the loop `ForPrep` set up and jumps back into the body while it has not ended. |

<!-- end of opcode table -->
