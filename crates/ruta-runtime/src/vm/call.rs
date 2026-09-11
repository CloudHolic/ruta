//! Entering and leaving a frame.

use ruta_bytecode::Vararg;

use crate::heap::Function;
use crate::stack::{Frame, Want};
use crate::value::Value;

use super::error::Error;
use super::state::Vm;

/// Deep enough that no honest program reaches it, shallow enough to s top before the host runs out of memory.
const DEPTH: usize = 200_000;

/// Calls whatever sits at `callee`, with `args` arguments just above it.
/// Anwers whether a Lua frame was pushed - a native has already finished when this returns.
pub(super) fn call(vm: &mut Vm, callee: u32, args: u16, want: Want) -> Result<bool, Error> {
    match vm.stack.at(callee) {
        Value::Func(handle) => match vm.heap.func(handle) {
            Function::Lua { .. } => {
                enter(vm, callee, args, want)?;

                Ok(true)
            }
            Function::Native { call, .. } => {
                let call = *call;
                let produced = call(vm, callee, args)?;

                settle(vm, callee, produced, want);
                Ok(false)
            }
        },
        other => Err(vm.throw(format!("attempt to call a {} value", other.type_name()))),
    }
}

pub(super) fn enter(vm: &mut Vm, callee: u32, args: u16, want: Want) -> Result<(), Error> {
    if vm.stack.depth() >= DEPTH {
        return Err(vm.throw("stack overflow".to_owned()));
    }

    let Value::Func(handle) = vm.stack.at(callee) else {
        unreachable!("the caller checked this");
    };

    let Function::Lua { proto, .. } = vm.heap.func(handle) else {
        unreachable!("the caller checked this");
    };

    let proto = *proto;
    let held = vm.heap.proto(proto);
    let params = u32::from(held.params);
    let registers = u32::from(held.max_registers);
    let kind = held.vararg;

    let args = u32::from(args);
    let first = callee + 1;

    // A funtion that reads `...` keeps its extra arguments below the frame,
    // so the fixed parameters move up past them.
    let (base, varargs) = match kind {
        Vararg::None => (first, 0),
        Vararg::Anonymous | Vararg::Table => (first + args, args.saturating_sub(params)),
    };

    let fixed = match kind {
        Vararg::Table => params + 1,
        _ => params,
    };

    vm.stack.reserve(base + registers.max(fixed));

    if base != first {
        let moved = params.min(args);
        vm.stack.shift(first, base, moved);
        vm.stack.fill(base + moved, base + params, Value::Nil);
    } else {
        vm.stack
            .fill(first + args.min(params), base + params, Value::Nil);
    }

    if kind == Vararg::Table {
        let table = vm.heap.new_table(varargs as usize, 0);

        for index in 0..varargs {
            let value = vm.stack.at(base - varargs + index);
            vm.heap
                .table_set(table, Value::Int(i64::from(index) + 1), value)
                .expect("a positive integer key")
        }

        vm.stack.put(base + params, Value::Table(table));
    }

    vm.stack.fill(base + fixed, base + registers, Value::Nil);

    vm.stack.enter(Frame::Lua {
        func: handle,
        base,
        top: base + registers,
        pc: 0,
        want,
        ret_to: callee,
        varargs,
    });

    Ok(())
}

/// Moves `produced` results sitting at `from` down to `to`, trimmed or padded to `want`.
pub(super) fn settle(vm: &mut Vm, to: u32, produced: u16, want: Want) -> u32 {
    let produced = u32::from(produced);
    let kept = match want {
        Want::All => produced,
        Want::Exactly(count) => u32::from(count),
    };

    vm.stack.reserve(to + kept);
    vm.stack
        .fill(to + produced.min(kept), to + kept, Value::Nil);

    kept
}
