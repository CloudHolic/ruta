//! AST in, IR out.

mod expr;
mod func;
mod stat;

pub use func::lower;
