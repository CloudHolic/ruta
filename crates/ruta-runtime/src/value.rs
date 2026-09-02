//! One Lua value, and the handles that name the objects on the heap.

use crate::heap::{FuncRef, StrRef, TableRef, ThreadRef, UserDataRef};

/// A Lua value.
#[derive(Debug, Clone, Copy)]
pub enum Value {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_two_words() {
        // The register stack of stage 4 is an array of these. Re-measure when a variant is
        // added; a bigger value makes every frame heavier.
        assert_eq!(size_of::<Value>(), 16);
    }

    #[test]
    fn type_names_are_the_ones_lua_prints() {
        assert_eq!(Value::Nil.type_name(), "nil");
        assert_eq!(Value::Bool(true).type_name(), "boolean");
        assert_eq!(Value::Int(1).type_name(), "number");
        assert_eq!(Value::Float(1.0).type_name(), "number");
    }

    #[test]
    fn only_nil_and_false_are_falsy() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(0).is_truthy());
        assert!(Value::Float(0.0).is_truthy());
    }
}
