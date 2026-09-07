//! A thin linear IR.

mod block;
mod func;
mod instr;

pub use block::{Block, BlockIdx};
pub use func::{FuncIdx, Function, Position, Program, Slot, UpvalSource, Vararg};
pub use instr::{BinOp, Const, Instr, Op, Reg, Results, UnOp};
