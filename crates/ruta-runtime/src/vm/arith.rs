//! The operators, on values alone.

use std::cmp::Ordering;

use crate::value::Value;

use super::error::Error;
use super::origin;
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

/// A value and the register it was read from, which an error message may name.
#[derive(Debug, Clone, Copy)]
pub(super) struct Operand {
    pub(super) value: Value,
    pub(super) reg: u8,
}

/// A number in whichever of the two shapes it arrived as.
#[derive(Debug, Clone, Copy)]
enum Num {
    Int(i64),
    Float(f64),
}

pub(super) fn arith(
    vm: &mut Vm,
    kind: Arith,
    left: Operand,
    right: Operand,
) -> Result<Value, Error> {
    let (Some(a), Some(b)) = (number(left.value), number(right.value)) else {
        return Err(arith_error(vm, left, right));
    };

    match (kind, a, b) {
        (Arith::Div, _, _) => Ok(Value::Float(float(a) / float(b))),
        (Arith::Pow, _, _) => Ok(Value::Float(float(a).powf(float(b)))),
        (Arith::Add, Num::Int(a), Num::Int(b)) => Ok(Value::Int(a.wrapping_add(b))),
        (Arith::Sub, Num::Int(a), Num::Int(b)) => Ok(Value::Int(a.wrapping_sub(b))),
        (Arith::Mul, Num::Int(a), Num::Int(b)) => Ok(Value::Int(a.wrapping_mul(b))),
        (Arith::IDiv, Num::Int(a), Num::Int(b)) => match b {
            0 => Err(vm.throw("attempt to divide by zero")),
            _ => Ok(Value::Int(floor_div(a, b))),
        },
        (Arith::Mod, Num::Int(a), Num::Int(b)) => match b {
            0 => Err(vm.throw("attempt to perform 'n%0'")),
            _ => Ok(Value::Int(floor_mod(a, b))),
        },
        (Arith::Add, _, _) => Ok(Value::Float(float(a) + float(b))),
        (Arith::Sub, _, _) => Ok(Value::Float(float(a) - float(b))),
        (Arith::Mul, _, _) => Ok(Value::Float(float(a) * float(b))),
        (Arith::IDiv, _, _) => Ok(Value::Float((float(a) / float(b)).floor())),
        (Arith::Mod, _, _) => Ok(Value::Float(float_mod(float(a), float(b)))),
    }
}

pub(super) fn negate(vm: &mut Vm, operand: Operand) -> Result<Value, Error> {
    match number(operand.value) {
        Some(Num::Int(number)) => Ok(Value::Int(number.wrapping_neg())),
        Some(Num::Float(number)) => Ok(Value::Float(-number)),
        None => Err(arith_error(vm, operand, operand)),
    }
}

pub(super) fn bitwise(
    vm: &mut Vm,
    kind: Bitwise,
    left: Operand,
    right: Operand,
) -> Result<Value, Error> {
    let (Some(a), Some(b)) = (integer(left.value), integer(right.value)) else {
        return Err(bitwise_error(vm, left, right));
    };

    Ok(Value::Int(match kind {
        Bitwise::And => a & b,
        Bitwise::Or => a | b,
        Bitwise::Xor => a ^ b,
        Bitwise::Shl => shift(a, b),
        Bitwise::Shr => shift(a, b.wrapping_neg()),
    }))
}

pub(super) fn complement(vm: &mut Vm, operand: Operand) -> Result<Value, Error> {
    match integer(operand.value) {
        Some(number) => Ok(Value::Int(!number)),
        None => Err(bitwise_error(vm, operand, operand)),
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
            _ => {
                let (left, right) = (left.type_name(), right.type_name());
                return Err(vm.throw(match left == right {
                    true => format!("attempt to compare two {left} values"),
                    false => format!("attempt to compare {left} with {right}"),
                }));
            }
        },
    };

    Ok(match kind {
        Compare::Lt | Compare::Gt => ordering == Some(Ordering::Less),
        _ => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
    })
}

pub(super) fn concat(vm: &mut Vm, left: Operand, right: Operand) -> Result<Value, Error> {
    let (Some(mut bytes), Some(tail)) = (text(vm, left.value), text(vm, right.value)) else {
        let bad = match text(vm, left.value) {
            Some(_) => right,
            None => left,
        };

        return Err(vm.fault("concatenate", bad.value, bad.reg));
    };

    bytes.extend_from_slice(&tail);

    Ok(Value::Str(vm.intern(&bytes)))
}

pub(super) fn length(vm: &mut Vm, operand: Operand) -> Result<Value, Error> {
    match operand.value {
        Value::Str(handle) => Ok(Value::Int(vm.bytes(handle).len() as i64)),
        Value::Table(handle) => Ok(Value::Int(vm.heap.table_length(handle))),
        other => Err(vm.fault("get length of", other, operand.reg)),
    }
}

/// A value the bitwise operators can use, which a float only is when it is whole.
pub(super) fn integer(value: Value) -> Option<i64> {
    match value {
        Value::Int(number) => Some(number),
        Value::Float(number) if number.floor() == number && whole(number) => Some(number as i64),
        _ => None,
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

/// The first operand that is not a number takes the blame.
fn arith_error(vm: &mut Vm, left: Operand, right: Operand) -> Error {
    let bad = match number(left.value) {
        Some(_) => right,
        None => left,
    };

    vm.fault("perform arithmetic on", bad.value, bad.reg)
}

/// Two numbers fail only for want of an integer, and the first that lacks one takes the
/// blame. Otherwise the first operand that is not a number does.
fn bitwise_error(vm: &mut Vm, left: Operand, right: Operand) -> Error {
    if number(left.value).is_some() && number(right.value).is_some() {
        let bad = match integer(left.value) {
            Some(_) => right,
            None => left,
        };

        let mut message = b"number".to_vec();
        message.extend(origin::clause(origin::name(vm, bad.reg)));
        message.extend_from_slice(b" has no integer representation");

        return vm.throw(message);
    }

    let bad = match number(left.value) {
        Some(_) => right,
        None => left,
    };

    vm.fault("perform bitwise operation on", bad.value, bad.reg)
}
