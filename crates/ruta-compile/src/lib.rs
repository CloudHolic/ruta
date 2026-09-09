//! The compiler: AST in IR out, and eventually bytecode.

mod alloc;
mod emit;
mod lower;

pub mod ir;

pub use alloc::allocate;
pub use emit::{Source, emit};
pub use lower::lower;
