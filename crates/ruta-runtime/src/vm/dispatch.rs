//! The loop. Frame state lives on the frame stack, never in a local that outlives a step.

use ruta_bytecode::{MULTI, Op, decode};

use crate::heap::{Function, Upvalue};
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
            debug_assert!(args != MULTI, "a spread call needs the pending-row mark");

            let want = match results {
                MULTI => Want::All,
                count => Want::Exactly(u16::from(count)),
            };

            call::call(vm, base + u32::from(callee), u16::from(args), want)?;
        }
        Op::Return { first, count } => {
            debug_assert!(count != MULTI, "a spread return needs the pending-row mark");

            let Some(Frame::Lua { want, ret_to, .. }) = vm.stack.leave() else {
                unreachable!("a frame was running");
            };

            let from = base + u32::from(first);
            let produced = u32::from(count);

            for index in 0..produced {
                let value = vm.stack.at(from + index);
                vm.stack.put(ret_to + index, value);
            }

            call::settle(vm, ret_to, u16::from(count), want);
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
        other => unimplemented!("{other:?}"),
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
