//! The heap: one arena, and handles that index it.

mod arena;
mod function;
mod handle;
mod proto;
mod string;
mod table;

pub use arena::Heap;
pub use function::{Function, Native, Upvalue};
pub use handle::{FuncRef, ProtoRef, StrRef, TableRef, ThreadRef, UpvalRef, UserDataRef};
pub use proto::Proto;
pub use string::LuaStr;
pub use table::{InvalidKey, KeyError};
