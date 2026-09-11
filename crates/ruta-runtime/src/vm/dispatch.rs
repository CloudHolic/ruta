//! The loop. Frame state lives on the frame stack, never in a local that outlives a step.

use ruta_bytecode::{MULTI, Op, decode};

use crate::heap::{Function, Upvalue};
use crate::stack::{Frame, Want};
use crate::value::Value;

use super::call;
use super::error::Error;
use super::state::Vm;

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
        other => unimplemented!("{other:?}"),
    }

    Ok(())
}

fn advance(vm: &mut Vm, len: u32) {
    let Frame::Lua { pc, .. } = vm.stack.current_mut();

    *pc += len;
}
