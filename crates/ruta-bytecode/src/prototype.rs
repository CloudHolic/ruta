//! Compiled functions.

use crate::constant::Constant;
use crate::debug::{LineTable, LocalVar};

/// What `...` was written as, which changes how the prologue binds the extra arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vararg {
    None,
    /// `...` - the extra arguments stay on the stack and [`crate::Op::Vararg`] reads them.
    Anonymous,
    /// `...t` - the extra arguments arrive as a table, bound like a parameter.
    Table,
}

/// Where an upvalue's value comes from, named in the enclosing function's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpvalSource {
    ParentLocal(u8),
    ParentUpval(u8),
}

#[derive(Debug)]
pub struct UpvalDesc {
    pub source: UpvalSource,
    pub name: Box<[u8]>,
}

/// One compiled function, with everything it needs  to run and to be reported on.
#[derive(Debug)]
pub struct Prototype {
    pub params: u8,
    pub vararg: Vararg,
    pub max_registers: u8,
    pub code: Box<[u8]>,
    pub constants: Box<[Constant]>,
    pub children: Box<[Prototype]>,
    pub upvals: Box<[UpvalDesc]>,
    /// The chunk name, already formatted. Every prototype carries its own, so that any one of them can be dumped on its own.
    pub source: Box<[u8]>,
    /// 0 for a chunk's outermost function.
    pub line_defined: u32,
    pub last_line_defined: u32,
    pub lines: LineTable,
    pub locals: Box<[LocalVar]>,
}
