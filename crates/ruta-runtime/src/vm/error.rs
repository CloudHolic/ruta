//! What a running program throws.

use crate::value::Value;

/// A value on its way out of the program.
/// Lua errors are values, not a Rust error type: `error({code = 1})` has to survive the trip.
#[derive(Debug)]
pub struct Error {
    pub value: Value,
}
