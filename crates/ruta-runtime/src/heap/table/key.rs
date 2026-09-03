//! What a table key is, and when two of them are the same one.

use crate::value::Value;

use super::super::arena::Heap;
use super::super::string::hash_bytes;

/// Why a value cannot be a table key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    Nil,
    Nan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidKey;

impl Heap {
    /// The hash of a normalized key.
    pub(super) fn key_hash(&self, key: Value) -> u32 {
        match key {
            Value::Nil => unreachable!("normalize refuses nil"),
            Value::Bool(flag) => hash_bytes(&[flag as u8]),
            Value::Int(number) => hash_bytes(&number.to_le_bytes()),
            Value::Float(number) => hash_bytes(&number.to_bits().to_le_bytes()),
            Value::Str(handle) => self.string(handle).hash(),
            Value::Table(handle) => hash_bytes(&handle.0.to_le_bytes()),
            Value::Func(handle) => hash_bytes(&handle.0.to_le_bytes()),
            Value::UserData(handle) => hash_bytes(&handle.0.to_le_bytes()),
            Value::Thread(handle) => hash_bytes(&handle.0.to_le_bytes()),
        }
    }

    /// Whether two normalized keys name the same slot.
    pub(super) fn keys_equal(&self, left: Value, right: Value) -> bool {
        match (left, right) {
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => {
                a == b || self.string(a).as_bytes() == self.string(b).as_bytes()
            }
            (Value::Table(a), Value::Table(b)) => a == b,
            (Value::Func(a), Value::Func(b)) => a == b,
            (Value::UserData(a), Value::UserData(b)) => a == b,
            (Value::Thread(a), Value::Thread(b)) => a == b,
            _ => false,
        }
    }
}

/// The key a value stands for, or why it cannot be one.
pub(super) fn normalize(key: Value) -> Result<Value, KeyError> {
    match key {
        Value::Nil => Err(KeyError::Nil),
        Value::Float(number) if number.is_nan() => Err(KeyError::Nan),
        Value::Float(number) => Ok(match integral(number) {
            Some(integer) => Value::Int(integer),
            None => Value::Float(number),
        }),
        other => Ok(other),
    }
}

/// The integer a float names exactly, if there is one.
fn integral(number: f64) -> Option<i64> {
    const LOWEST: f64 = -9_223_372_036_854_775_808.0;
    const PAST_HIGHEST: f64 = 9_223_372_036_854_775_808.0;

    if number.floor() != number || !(LOWEST..PAST_HIGHEST).contains(&number) {
        return None;
    }

    Some(number as i64)
}
