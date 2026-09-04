//! The bytecode a chunk compiles to.

mod constant;
mod debug;
mod decode;
mod encode;
mod op;
mod prototype;

pub use constant::Constant;
pub use debug::{LineTable, LocalVar};
pub use decode::{DecodeError, decode};
pub use encode::Encoder;
pub use op::{MULTI, Op, OpCode, Operand, instruction_len};
pub use prototype::{Prototype, UpvalDesc, UpvalSource, Vararg};
