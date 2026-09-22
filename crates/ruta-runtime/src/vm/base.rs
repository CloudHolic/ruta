//! The library a chunk finds in its globals.

use std::io::{self, Write};

use crate::heap::{FuncRef, Heap, Native};
use crate::value::Value;

use super::arith;
use super::error::Error;
use super::state::Vm;

#[cfg(windows)]
const LINE_END: &[u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_END: &[u8] = b"\n";

/// The iterators `pairs` and `ipairs` hand out.
pub(super) fn iterators(heap: &mut Heap) -> (FuncRef, FuncRef) {
    (
        heap.new_native(b"next", next),
        heap.new_native(b"for iterator", step),
    )
}

pub(super) fn install(vm: &mut Vm) {
    let next = vm.next;

    set(vm, b"next", next);
    define(vm, b"assert", assert);
    define(vm, b"ipairs", ipairs);
    define(vm, b"pairs", pairs);
    define(vm, b"print", print);
    define(vm, b"select", select);
    define(vm, b"tonumber", tonumber);
    define(vm, b"tostring", tostring);
    define(vm, b"type", r#type);
}

fn define(vm: &mut Vm, name: &[u8], call: Native) {
    let handle = vm.heap.new_native(name, call);

    set(vm, name, handle);
}

fn set(vm: &mut Vm, name: &[u8], handle: FuncRef) {
    let key = vm.heap.new_string(name);

    vm.heap
        .table_set(vm.globals, Value::Str(key), Value::Func(handle))
        .expect("a string key");
}

fn assert(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    if any(vm, callee, args, 1)?.is_truthy() {
        for index in 1..=args {
            let value = vm.stack.at(callee + index);
            vm.stack.push(value);
        }

        return Ok(args);
    }

    // A message that is a string says where it was raised; any other value travels as it is.
    match arg(vm, callee, args, 2) {
        None => Err(vm.throw("assertion failed!")),
        Some(Value::Str(handle)) => {
            let message = vm.bytes(handle).to_vec();

            Err(vm.throw(message))
        }
        Some(value) => Err(Error { value }),
    }
}

fn ipairs(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let table = any(vm, callee, args, 1)?;
    let step = vm.ipairs_step;

    vm.stack.push(Value::Func(step));
    vm.stack.push(table);
    vm.stack.push(Value::Int(0));

    Ok(3)
}

/// The entry after `index`, or a single nil once the sequence ends.
fn step(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let table = arg(vm, callee, args, 1).unwrap_or(Value::Nil);
    let index = integer(vm, callee, args, 2)?.wrapping_add(1);

    let value = match table {
        Value::Table(table) => vm.heap.table_get(table, Value::Int(index)),
        other => {
            return Err(vm.bare(format!("attempt to index a {} value", other.type_name())));
        }
    };

    if matches!(value, Value::Nil) {
        vm.stack.push(Value::Nil);

        return Ok(1);
    }

    vm.stack.push(Value::Int(index));
    vm.stack.push(value);

    Ok(2)
}

fn next(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let table = match arg(vm, callee, args, 1) {
        Some(Value::Table(table)) => table,
        other => {
            return Err(vm.bad_argument(callee, 1, &format!("table expected, got {}", got(other))));
        }
    };
    let key = arg(vm, callee, args, 2).unwrap_or(Value::Nil);

    match vm.heap.table_next(table, key) {
        Ok(Some((key, value))) => {
            vm.stack.push(key);
            vm.stack.push(value);

            Ok(2)
        }
        Ok(None) => {
            vm.stack.push(Value::Nil);

            Ok(1)
        }
        Err(_) => Err(vm.bare("invalid key to 'next'")),
    }
}

fn pairs(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let table = any(vm, callee, args, 1)?;
    let next = vm.next;

    vm.stack.push(Value::Func(next));
    vm.stack.push(table);
    vm.stack.push(Value::Nil);

    Ok(3)
}

fn print(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let mut line = Vec::new();

    for index in 0..args {
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

fn select(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    if let Some(Value::Str(handle)) = arg(vm, callee, args, 1)
        && vm.bytes(handle).first() == Some(&b'#')
    {
        vm.stack.push(Value::Int(i64::from(args) - 1));

        return Ok(1);
    }

    let asked = integer(vm, callee, args, 1)?;
    let top = i64::from(args);
    let from = match asked < 0 {
        true => top.saturating_add(asked),
        false => asked.min(top),
    };

    if from < 1 {
        return Err(vm.bad_argument(callee, 1, "index out of range"));
    }

    for index in from + 1..=top {
        let value = vm.stack.at(callee + index as u32);
        vm.stack.push(value);
    }

    Ok((top - from) as u32)
}

fn tonumber(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let number = match arg(vm, callee, args, 2) {
        None | Some(Value::Nil) => {
            let value = any(vm, callee, args, 1)?;

            vm.to_number(value).unwrap_or(Value::Nil)
        }
        Some(_) => {
            let base = integer(vm, callee, args, 2)?;
            let given = arg(vm, callee, args, 1);

            let Some(Value::Str(text)) = given else {
                return Err(vm.bad_argument(
                    callee,
                    1,
                    &format!("string expected, got {}", got(given)),
                ));
            };

            if !(2..=36).contains(&base) {
                return Err(vm.bad_argument(callee, 2, "base out of range"));
            }

            in_base(vm.bytes(text), base as u32).map_or(Value::Nil, Value::Int)
        }
    };

    vm.stack.push(number);

    Ok(1)
}

fn tostring(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let value = any(vm, callee, args, 1)?;
    let text = vm.text(value);
    let handle = vm.intern(&text);

    vm.stack.push(Value::Str(handle));

    Ok(1)
}

fn r#type(vm: &mut Vm, callee: u32, args: u32) -> Result<u32, Error> {
    let value = any(vm, callee, args, 1)?;
    let name = vm.intern(value.type_name().as_bytes());

    vm.stack.push(Value::Str(name));

    Ok(1)
}

/// The argument at `index`, counting from one, or `None` past the last one passed.
fn arg(vm: &Vm, callee: u32, args: u32, index: u32) -> Option<Value> {
    (index <= args).then(|| vm.stack.at(callee + index))
}

/// An argument that has to be there, though it may be nil.
fn any(vm: &mut Vm, callee: u32, args: u32, index: u32) -> Result<Value, Error> {
    match arg(vm, callee, args, index) {
        Some(value) => Ok(value),
        None => Err(vm.bad_argument(callee, index, "value expected")),
    }
}

/// An argument that has to be a whole number, which a string spelling one also is.
fn integer(vm: &mut Vm, callee: u32, args: u32, index: u32) -> Result<i64, Error> {
    let given = arg(vm, callee, args, index);

    match given.and_then(|value| vm.to_number(value)) {
        Some(number) => arith::integer(number)
            .ok_or_else(|| vm.bad_argument(callee, index, "number has no integer representation")),
        None => Err(vm.bad_argument(
            callee,
            index,
            &format!("number expected, got {}", got(given)),
        )),
    }
}

fn got(value: Option<Value>) -> &'static str {
    value.map_or("no value", |value| value.type_name())
}

/// Digits and letters in `base`, with surrounding whitespace and one sign. A value too large
/// for an integer wraps around.
fn in_base(text: &[u8], base: u32) -> Option<i64> {
    let space = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r');
    let start = text
        .iter()
        .position(|byte| !space(byte))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|byte| !space(byte))
        .map_or(start, |at| at + 1);

    let (negative, digits) = match &text[start..end] {
        [b'-', rest @ ..] => (true, rest),
        [b'+', rest @ ..] => (false, rest),
        rest => (false, rest),
    };

    if digits.is_empty() {
        return None;
    }

    let mut value: i64 = 0;

    for byte in digits {
        let digit = char::from(*byte)
            .to_digit(36)
            .filter(|digit| *digit < base)?;
        value = value
            .wrapping_mul(i64::from(base))
            .wrapping_add(i64::from(digit));
    }

    Some(match negative {
        true => value.wrapping_neg(),
        false => value,
    })
}
