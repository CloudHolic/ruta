//! The heap: one arena, and handles that index it.

mod arena;
mod handle;
mod string;
mod table;

pub use arena::Heap;
pub use handle::{FuncRef, StrRef, TableRef, ThreadRef, UserDataRef};
pub use string::LuaStr;
pub use table::{InvalidKey, KeyError};
