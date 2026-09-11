//! The library a chunk finds in its globals.

use std::io::{self, Write};

use crate::heap::Native;
use crate::value::Value;

use super::error::Error;
use super::state::Vm;

#[cfg(windows)]
const LINE_END: &[u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_END: &[u8] = b"\n";

pub(super) fn install(vm: &mut Vm) {
    define(vm, b"print", print);
}

fn define(vm: &mut Vm, name: &[u8], call: Native) {
    let handle = vm.heap.new_native(name, call);
    let key = vm.heap.new_string(name);

    vm.heap
        .table_set(vm.globals, Value::Str(key), Value::Func(handle))
        .expect("a string key");
}

fn print(vm: &mut Vm, callee: u32, args: u16) -> Result<u16, Error> {
    let mut line = Vec::new();

    for index in 0..u32::from(args) {
        if index > 0 {
            line.push(b'\t');
        }

        let value = vm.stack.at(callee + 1 + index);
        line.extend_from_slice(&vm.text(value));
    }
    line.extend_from_slice(LINE_END);

    // Nothing is left to report to if stdout itself cannot be written.
    let _ = io::stdout().write_all(&line);

    Ok(0)
}
