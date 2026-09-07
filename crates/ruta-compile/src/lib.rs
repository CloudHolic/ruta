//! The compiler: AST in IR out, and eventually bytecode.

mod alloc;
mod lower;

pub mod ir;

pub use alloc::allocate;
pub use lower::lower;
