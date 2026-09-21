//! The operators, on values alone.

use std::cmp::Ordering;

use crate::value::Value;

use super::error::Error;
use super::state::Vm;

#[derive(Debug, Clone, Copy)]
pub(super) enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    Pow,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Bitwise {
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Compare {
    Lt,
    Le,
    Gt,
    Ge,
}

/// A number in whichever of the two shapes it arrived as.
#[derive(Debug, Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

pub(super) fn arith(vm: &mut Vm, kind: Arith, left: Value, right: Value) -> Result<Value, Error> {
    let (Some(left), Some(right)) = (number(left), number(right)) else {
        return Err(arith_error(vm, left, right));
    };

    match (kind, left, right) {
        (Arith::Div, _, _) => Ok(Value::Float(float(left) / float(right))),
        (Arith::Pow, _, _) => Ok(Value::Float(float(left).powf(float(right)))),
        (Arith::Add, Num::Int(left), Num::Int(right)) => Ok(Value::Int(left.wrapping_add(right))),
        (Arith::Sub, Num::Int(left), Num::Int(right)) => Ok(Value::Int(left.wrapping_sub(right))),
        (Arith::Mul, Num::Int(left), Num::Int(right)) => Ok(Value::Int(left.wrapping_mul(right))),
        (Arith::IDiv, Num::Int(left), Num::Int(right)) => match right {
            0 => Err(vm.throw("attempt to divide by zero".to_owned())),
            _ => Ok(Value::Int(floor_div(left, right))),
        },
        (Arith::Mod, Num::Int(left), Num::Int(right)) => match right {
            0 => Err(vm.throw("attempt to perform 'n%0'".to_owned())),
            _ => Ok(Value::Int(floor_mod(left, right))),
        },
        (Arith::Add, _, _) => Ok(Value::Float(float(left) + float(right))),
        (Arith::Sub, _, _) => Ok(Value::Float(float(left) - float(right))),
        (Arith::Mul, _, _) => Ok(Value::Float(float(left) * float(right))),
        (Arith::IDiv, _, _) => Ok(Value::Float((float(left) / float(right)).floor())),
        (Arith::Mod, _, _) => Ok(Value::Float(float_mod(float(left), float(right)))),
    }
}

pub(super) fn negate(vm: &mut Vm, value: Value) -> Result<Value, Error> {
    match number(value) {
        Some(Num::Int(number)) => Ok(Value::Int(number.wrapping_neg())),
        Some(Num::Float(number)) => Ok(Value::Float(-number)),
        None => Err(arith_error(vm, value, value)),
    }
}

pub(super) fn bitwise(
    vm: &mut Vm,
    kind: Bitwise,
    left: Value,
    right: Value,
) -> Result<Value, Error> {
    let (Some(left), Some(right)) = (integer(left), integer(right)) else {
        return Err(bitwise_error(vm, left, right));
    };

    Ok(Value::Int(match kind {
        Bitwise::And => left & right,
        Bitwise::Or => left | right,
        Bitwise::Xor => left ^ right,
        Bitwise::Shl => shift(left, right),
        Bitwise::Shr => shift(left, right.wrapping_neg()),
    }))
}

pub(super) fn complement(vm: &mut Vm, value: Value) -> Result<Value, Error> {
    match integer(value) {
        Some(number) => Ok(Value::Int(!number)),
        None => Err(bitwise_error(vm, value, value)),
    }
}

pub(super) fn equal(vm: &Vm, left: Value, right: Value) -> bool {
    match (left, right) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Str(left), Value::Str(right)) => left == right || vm.bytes(left) == vm.bytes(right),
        (Value::Table(left), Value::Table(right)) => left == right,
        (Value::Func(left), Value::Func(right)) => left == right,
        (Value::UserData(left), Value::UserData(right)) => left == right,
        (Value::Thread(left), Value::Thread(right)) => left == right,
        _ => match (number(left), number(right)) {
            (Some(left), Some(right)) => order(left, right) == Some(Ordering::Equal),
            _ => false,
        },
    }
}

pub(super) fn compare(
    vm: &mut Vm,
    kind: Compare,
    left: Value,
    right: Value,
) -> Result<bool, Error> {
    let (left, right) = match kind {
        Compare::Gt | Compare::Ge => (right, left),
        _ => (left, right),
    };

    let ordering = match (left, right) {
        (Value::Str(left), Value::Str(right)) => Some(vm.bytes(left).cmp(vm.bytes(right))),
        _ => match (number(left), number(right)) {
            (Some(left), Some(right)) => order(left, right),
            _ => return Err(compare_error(vm, left, right)),
        },
    };

    Ok(match kind {
        Compare::Lt | Compare::Gt => ordering == Some(Ordering::Less),
        _ => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
    })
}

pub(super) fn concat(vm: &mut Vm, left: Value, right: Value) -> Result<Value, Error> {
    let (Some(mut bytes), Some(tail)) = (text(vm, left), text(vm, right)) else {
        let bad = match text(vm, left) {
            Some(_) => right,
            None => left,
        };

        return Err(vm.throw(format!(
            "attempt to concatenate a {} value",
            bad.type_name()
        )));
    };

    bytes.extend_from_slice(&tail);

    Ok(Value::Str(vm.intern(&bytes)))
}

pub(super) fn length(vm: &mut Vm, value: Value) -> Result<Value, Error> {
    match value {
        Value::Str(handle) => Ok(Value::Int(vm.bytes(handle).len() as i64)),
        Value::Table(handle) => Ok(Value::Int(vm.heap.table_length(handle))),
        other => Err(vm.throw(format!(
            "attempt to get length of a {} value",
            other.type_name()
        ))),
    }
}

/// Only numbers here: a string that looks like one becomes one through the string
/// metatable, which arrives with metamethods.
fn number(value: Value) -> Option<Num> {
    match value {
        Value::Int(number) => Some(Num::Int(number)),
        Value::Float(number) => Some(Num::Float(number)),
        _ => None,
    }
}

/// A value the bitwise operators can use, which a float only is when it is whole.
fn integer(value: Value) -> Option<i64> {
    match value {
        Value::Int(number) => Some(number),
        Value::Float(number) if number.floor() == number && whole(number) => Some(number as i64),
        _ => None,
    }
}

fn whole(number: f64) -> bool {
    number >= -(2.0_f64.powi(63)) && number < 2.0_f64.powi(63)
}

fn float(number: Num) -> f64 {
    match number {
        Num::Int(number) => number as f64,
        Num::Float(number) => number,
    }
}

fn floor_div(left: i64, right: i64) -> i64 {
    let quotient = left.wrapping_div(right);

    match left.wrapping_rem(right) != 0 && (left < 0) != (right < 0) {
        true => quotient - 1,
        false => quotient,
    }
}

fn floor_mod(left: i64, right: i64) -> i64 {
    let rest = left.wrapping_rem(right);

    match rest != 0 && (rest < 0) != (right < 0) {
        true => rest + right,
        false => rest,
    }
}

fn float_mod(left: f64, right: f64) -> f64 {
    let rest = left % right;

    match rest != 0.0 && (rest < 0.0) != (right < 0.0) {
        true => rest + right,
        false => rest,
    }
}

/// Shifts by more than the width give zero, and a negative count goes the other way.
fn shift(value: i64, places: i64) -> i64 {
    if places <= -64 || places >= 64 {
        return 0;
    }

    match places >= 0 {
        true => ((value as u64) << places) as i64,
        false => ((value as u64) >> -places) as i64,
    }
}

/// Exact across the two shapes: a large integer must not lose its low bits to `f64`.
fn order(left: Num, right: Num) -> Option<Ordering> {
    match (left, right) {
        (Num::Int(left), Num::Int(right)) => Some(left.cmp(&right)),
        (Num::Float(left), Num::Float(right)) => left.partial_cmp(&right),
        (Num::Int(left), Num::Float(right)) => against(left, right),
        (Num::Float(left), Num::Int(right)) => against(right, left).map(Ordering::reverse),
    }
}

fn against(left: i64, right: f64) -> Option<Ordering> {
    if right.is_nan() {
        return None;
    }

    if right >= 2.0_f64.powi(63) {
        return Some(Ordering::Less);
    }

    if right < -(2.0_f64.powi(63)) {
        return Some(Ordering::Greater);
    }

    let floor = right.floor();

    Some(match left.cmp(&(floor as i64)) {
        Ordering::Equal if right > floor => Ordering::Less,
        other => other,
    })
}

fn text(vm: &Vm, value: Value) -> Option<Vec<u8>> {
    match value {
        Value::Str(handle) => Some(vm.bytes(handle).to_vec()),
        Value::Int(number) => Some(number.to_string().into_bytes()),
        Value::Float(number) => Some(crate::number::float(number).into_bytes()),
        _ => None,
    }
}

fn arith_error(vm: &mut Vm, left: Value, right: Value) -> Error {
    let bad = match number(left) {
        Some(_) => right,
        None => left,
    };

    vm.throw(format!(
        "attempt to perform arithmetic on a {} value",
        bad.type_name()
    ))
}

fn bitwise_error(vm: &mut Vm, left: Value, right: Value) -> Error {
    let bad = match integer(left) {
        Some(_) => right,
        None => left,
    };

    match number(bad) {
        Some(_) => vm.throw("number has no integer representation".to_owned()),
        None => vm.throw(format!(
            "attempt to perform bitwise operation on a {} value",
            bad.type_name()
        )),
    }
}

fn compare_error(vm: &mut Vm, left: Value, right: Value) -> Error {
    vm.throw(format!(
        "attempt to compare {} with {}",
        left.type_name(),
        right.type_name()
    ))
}
