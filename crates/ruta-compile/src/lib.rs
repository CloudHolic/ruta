//! The compiler: AST in IR out, and eventually bytecode.

mod lower;

pub mod ir;

pub use lower::lower;
