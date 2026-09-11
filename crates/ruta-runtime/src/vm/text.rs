//! What `tostring` answers.

use crate::number;
use crate::value::Value;

use super::state::Vm;

pub(super) fn of(vm: &mut Vm, value: Value) -> Vec<u8> {
    match value {
        Value::Nil => b"nil".to_vec(),
        Value::Bool(true) => b"true".to_vec(),
        Value::Bool(false) => b"false".to_vec(),
        Value::Int(number) => number.to_string().into_bytes(),
        Value::Float(number) => number::float(number).into_bytes(),
        Value::Str(handle) => vm.heap.string(handle).as_bytes().to_vec(),
        other => format!("{}: {:#014x}", other.type_name(), address(other)).into_bytes(),
    }
}

/// Lua shows an object's address.
/// ruta has handles instead, and nothing can assert on the digits either way.
fn address(value: Value) -> u64 {
    match value {
        Value::Table(handle) => u64::from(handle.index()),
        Value::Func(handle) => u64::from(handle.index()),
        Value::UserData(handle) => u64::from(handle.index()),
        Value::Thread(handle) => u64::from(handle.index()),
        _ => 0,
    }
}
