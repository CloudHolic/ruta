//! One Lua value, and the handles that name the objects on the heap.

use crate::heap::{FuncRef, StrRef, TableRef, ThreadRef, UserDataRef};

/// A Lua value.
#[derive(Debug, Clone, Copy, Default)]
pub enum Value {
    #[default]
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(StrRef),
    Table(TableRef),
    Func(FuncRef),
    UserData(UserDataRef),
    Thread(ThreadRef),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Int(_) | Value::Float(_) => "number",
            Value::Str(_) => "string",
            Value::Table(_) => "table",
            Value::Func(_) => "function",
            Value::UserData(_) => "userdata",
            Value::Thread(_) => "thread",
        }
    }

    /// Everything except `nil` and `false` is true in a condition.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }
}
