//! The parser.

mod chunk;
mod expr;
mod func;
mod near;
mod stat;

pub use chunk::parse_chunk;
