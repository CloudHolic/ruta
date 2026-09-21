//! The loop. Frame state lives on the frame stack, never in a local that outlives a step.

use ruta_bytecode::{MULTI, Op, UpvalSource, decode};

use crate::heap::{Function, KeyError, Upvalue};
use crate::stack::{Frame, Want};
use crate::value::Value;

use super::arith::{self, Arith, Bitwise, Compare};
use super::call;
use super::error::Error;
use super::loops;
use super::state::Vm;

#[derive(Debug, Clone, Copy)]
enum Kind {
    Arith(Arith),
    Bits(Bitwise),
    Order(Compare),
    Same(bool),
    Join,
}

pub(super) fn run(vm: &mut Vm) -> Result<(), Error> {
    while vm.stack.depth() > 0 {
        let (op, len) = fetch(vm);
        step(vm, op, len)?;
    }

    Ok(())
}

fn fetch(vm: &Vm) -> (Op, u32) {
    let Frame::Lua { func, pc, .. } = *vm.stack.current();
    let Function::Lua { proto, .. } = vm.heap.func(func) else {
        unreachable!("a Lua frame holds a Lua closure");
    };

    decode(&vm.heap.proto(*proto).code, pc).expect("code the emitter wrote")
}

fn step(vm: &mut Vm, op: Op, len: u32) -> Result<(), Error> {
    let Frame::Lua { base, .. } = *vm.stack.current();

    advance(vm, len);

    match op {
        Op::LoadConst { dest, constant } => {
            let value = vm.constant(constant);
            vm.stack.put(base + u32::from(dest), value);
        }
        Op::GetUpval { dest, index } => {
            let value = match vm.upvalue(usize::from(index)) {
                Upvalue::Open(slot) => vm.stack.at(slot),
                Upvalue::Closed(value) => value,
            };

            vm.stack.put(base + u32::from(dest), value);
        }
        Op::Index { dest, object, key } => {
            let object = vm.stack.at(base + u32::from(object));
            let key = vm.stack.at(base + u32::from(key));

            let value = match object {
                Value::Table(table) => vm.heap.table_get(table, key),
                other => {
                    return Err(vm.throw(format!("attempt to index a {} value", other.type_name())));
                }
            };

            vm.stack.put(base + u32::from(dest), value);
        }
        Op::Call {
            callee,
            args,
            results,
        } => {
            let callee = base + u32::from(callee);
            let args = match args {
                MULTI => vm.pending() - callee - 1,
                count => u32::from(count),
            };
            let want = match results {
                MULTI => Want::All,
                count => Want::Exactly(u16::from(count)),
            };

            vm.settle_top();
            call::call(vm, callee, args, want)?;
        }
        Op::TailCall { callee, args } => {
            let from = base + u32::from(callee);
            let args = match args {
                MULTI => vm.pending() - from - 1,
                count => u32::from(count),
            };

            // Checked while this frame still exists, so the message has a place to point at.
            let target = vm.stack.at(from);

            if !matches!(target, Value::Func(_)) {
                return Err(vm.throw(format!("attempt to call a {} value", target.type_name())));
            }

            vm.close(base);

            let Some(Frame::Lua { want, ret_to, .. }) = vm.stack.leave() else {
                unreachable!("a frame was running");
            };

            vm.stack.shift(from, ret_to, args + 1);
            call::call(vm, ret_to, args, want)?;
        }
        Op::Return { first, count } => {
            let from = base + u32::from(first);
            let produced = match count {
                MULTI => vm.pending() - from,
                count => u32::from(count),
            };

            // A returning frame's locals go away here, and nothing before this closes them.
            vm.close(base);

            let Some(Frame::Lua { want, ret_to, .. }) = vm.stack.leave() else {
                unreachable!("a frame was running");
            };

            call::settle(vm, from, ret_to, produced, want);
        }
        Op::Vararg { first, count } => {
            let Frame::Lua { varargs, .. } = *vm.stack.current();
            let to = base + u32::from(first);
            let wanted = match count {
                MULTI => varargs,
                count => u32::from(count),
            };
            let copied = wanted.min(varargs);

            vm.stack.reserve(to + wanted);
            vm.stack.shift(base - varargs, to, copied);
            vm.stack.fill(to + copied, to + wanted, Value::Nil);

            if count == MULTI {
                let Frame::Lua { top, .. } = vm.stack.current_mut();

                *top = to + wanted;
            }
        }
        Op::SetListSpread {
            table,
            first,
            first_index,
        } => {
            let Value::Table(handle) = vm.stack.at(base + u32::from(table)) else {
                unreachable!("a constructor fills the table it made");
            };
            let from = base + u32::from(first);

            for offset in 0..vm.pending() - from {
                let value = vm.stack.at(from + offset);
                let index = i64::from(first_index) + i64::from(offset);

                vm.heap
                    .table_set(handle, Value::Int(index), value)
                    .expect("a positive integer key");
            }

            vm.settle_top();
        }
        Op::LoadNil { dest } => vm.stack.put(base + u32::from(dest), Value::Nil),
        Op::LoadTrue { dest } => vm.stack.put(base + u32::from(dest), Value::Bool(true)),
        Op::LoadFalse { dest } => vm.stack.put(base + u32::from(dest), Value::Bool(false)),
        Op::Move { dest, src } => {
            let value = vm.stack.at(base + u32::from(src));
            vm.stack.put(base + u32::from(dest), value);
        }
        Op::SetUpval { index, src } => {
            let value = vm.stack.at(base + u32::from(src));

            match vm.upvalue(usize::from(index)) {
                Upvalue::Open(slot) => vm.stack.put(slot, value),
                Upvalue::Closed(_) => {
                    let cell = vm.cell(usize::from(index));
                    vm.heap.set_upvalue(cell, Upvalue::Closed(value));
                }
            }
        }
        Op::Neg { dest, operand } => {
            let value = vm.stack.at(base + u32::from(operand));
            let value = arith::negate(vm, value)?;
            vm.stack.put(base + u32::from(dest), value);
        }
        Op::Not { dest, operand } => {
            let value = vm.stack.at(base + u32::from(operand));
            vm.stack
                .put(base + u32::from(dest), Value::Bool(!value.is_truthy()));
        }
        Op::Len { dest, operand } => {
            let value = vm.stack.at(base + u32::from(operand));
            let value = arith::length(vm, value)?;
            vm.stack.put(base + u32::from(dest), value);
        }
        Op::BNot { dest, operand } => {
            let value = vm.stack.at(base + u32::from(operand));
            let value = arith::complement(vm, value)?;
            vm.stack.put(base + u32::from(dest), value);
        }
        Op::Add { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Add))?
        }
        Op::Sub { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Sub))?
        }
        Op::Mul { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Mul))?
        }
        Op::Div { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Div))?
        }
        Op::IDiv { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::IDiv))?
        }
        Op::Mod { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Mod))?
        }
        Op::Pow { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Arith(Arith::Pow))?
        }
        Op::BAnd { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Bits(Bitwise::And))?
        }
        Op::BOr { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Bits(Bitwise::Or))?
        }
        Op::BXor { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Bits(Bitwise::Xor))?
        }
        Op::Shl { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Bits(Bitwise::Shl))?
        }
        Op::Shr { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Bits(Bitwise::Shr))?
        }
        Op::Lt { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Order(Compare::Lt))?
        }
        Op::Le { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Order(Compare::Le))?
        }
        Op::Gt { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Order(Compare::Gt))?
        }
        Op::Ge { dest, left, right } => {
            binary(vm, base, dest, left, right, Kind::Order(Compare::Ge))?
        }
        Op::Eq { dest, left, right } => binary(vm, base, dest, left, right, Kind::Same(true))?,
        Op::Ne { dest, left, right } => binary(vm, base, dest, left, right, Kind::Same(false))?,
        Op::Concat { dest, left, right } => binary(vm, base, dest, left, right, Kind::Join)?,
        Op::Jump { offset } => jump(vm, offset),
        Op::JumpIfTrue { cond, offset } => {
            if vm.stack.at(base + u32::from(cond)).is_truthy() {
                jump(vm, offset);
            }
        }
        Op::JumpIfFalse { cond, offset } => {
            if !vm.stack.at(base + u32::from(cond)).is_truthy() {
                jump(vm, offset);
            }
        }
        Op::ForPrep { control, offset } => {
            if !loops::prepare(vm, base + u32::from(control))? {
                jump(vm, offset);
            }
        }
        Op::ForLoop { control, offset } => {
            if loops::advance(vm, base + u32::from(control)) {
                jump(vm, offset);
            }
        }
        Op::NewTable {
            dest,
            array_hint,
            hash_hint,
        } => {
            let table = vm.heap.new_table(array_hint as usize, hash_hint as usize);
            vm.stack.put(base + u32::from(dest), Value::Table(table));
        }
        Op::SetIndex { object, key, src } => {
            let object = vm.stack.at(base + u32::from(object));
            let key = vm.stack.at(base + u32::from(key));
            let value = vm.stack.at(base + u32::from(src));

            store(vm, object, key, value)?;
        }
        Op::DefineGlobal { env, key, src } => {
            let env = vm.stack.at(base + u32::from(env));
            let key = vm.stack.at(base + u32::from(key));
            let value = vm.stack.at(base + u32::from(src));

            // Only nil counts as undefined: a global holding false is already defined.
            if let Value::Table(table) = env
                && !matches!(vm.heap.table_get(table, key), Value::Nil)
            {
                let Value::Str(name) = key else {
                    unreachable!("a global is named by a string");
                };
                let name = String::from_utf8_lossy(vm.bytes(name)).into_owned();

                return Err(vm.throw(format!("global '{name}' already defined")));
            }

            store(vm, env, key, value)?;
        }
        Op::SetList {
            table,
            first,
            count,
            first_index,
        } => {
            let Value::Table(handle) = vm.stack.at(base + u32::from(table)) else {
                unreachable!("a constructor fills the table it made");
            };

            for offset in 0..u32::from(count) {
                let value = vm.stack.at(base + u32::from(first) + offset);

                vm.heap
                    .table_set(handle, Value::Int(i64::from(first_index + offset)), value)
                    .expect("a positive integer key");
            }
        }
        Op::Closure { dest, child } => {
            let parent = vm.running();
            let child = vm.heap.proto(parent).children[child as usize];
            let count = vm.heap.proto(child).upvals.len();
            let mut upvals = Vec::with_capacity(count);

            for index in 0..count {
                upvals.push(match vm.heap.proto(child).upvals[index].source {
                    UpvalSource::ParentLocal(reg) => vm.capture(base + u32::from(reg)),
                    UpvalSource::ParentUpval(at) => vm.cell(usize::from(at)),
                });
            }

            let closure = vm.heap.new_closure(child, upvals.into_boxed_slice());
            vm.stack.put(base + u32::from(dest), Value::Func(closure));
        }
        Op::CloseUpvals { from } => vm.close(base + u32::from(from)),
    }

    Ok(())
}

fn binary(vm: &mut Vm, base: u32, dest: u8, left: u8, right: u8, kind: Kind) -> Result<(), Error> {
    let left = vm.stack.at(base + u32::from(left));
    let right = vm.stack.at(base + u32::from(right));

    let value = match kind {
        Kind::Arith(kind) => arith::arith(vm, kind, left, right)?,
        Kind::Bits(kind) => arith::bitwise(vm, kind, left, right)?,
        Kind::Order(kind) => Value::Bool(arith::compare(vm, kind, left, right)?),
        Kind::Same(wanted) => Value::Bool(arith::equal(vm, left, right) == wanted),
        Kind::Join => arith::concat(vm, left, right)?,
    };

    vm.stack.put(base + u32::from(dest), value);

    Ok(())
}

fn advance(vm: &mut Vm, len: u32) {
    let Frame::Lua { pc, .. } = vm.stack.current_mut();

    *pc += len;
}

/// Offsets count from the instruction after the jump, which is where the pc already is.
fn jump(vm: &mut Vm, offset: i32) {
    let Frame::Lua { pc, .. } = vm.stack.current_mut();

    *pc = pc
        .checked_add_signed(offset)
        .expect("a jump the emitter kept inside the code");
}

fn store(vm: &mut Vm, object: Value, key: Value, value: Value) -> Result<(), Error> {
    let Value::Table(table) = object else {
        return Err(vm.throw(format!("attempt to index a {} value", object.type_name())));
    };

    vm.heap
        .table_set(table, key, value)
        .map_err(|error| match error {
            KeyError::Nil => vm.throw("table index is nil".to_owned()),
            KeyError::Nan => vm.throw("table index is NaN".to_owned()),
        })
}
