//! Turning  the IR into the bytecode a prototype carries.

mod chunk;
mod instr;
mod pool;

pub use chunk::{Source, emit};
