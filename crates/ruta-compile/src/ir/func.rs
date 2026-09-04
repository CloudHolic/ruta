//! Functions, and where each one's captured values come from.

use ruta_syntax::token::Span;

use super::block::Block;
use super::instr::Reg;

/// A whole chunk: every function it defines, main first.
#[derive(Debug, Default)]
pub struct Program {
    pub funcs: Vec<Function>,
}

/// One function body.
#[derive(Debug)]
pub struct Function {
    pub params: u16,
    pub vararg: Vararg,
    /// Indexed by [`BlockIdx`]. Entry 0 is where control enters.
    pub blocks: Vec<Block>,
    /// How many virtual registers were handed out. Register allocation maps these down.
    pub regs: u32,
    /// One entry per upvalue, in the order the body refers to them.
    pub upvalues: Vec<UpvalSource>,
    /// The whole body, for `in function at line %d` in a compile error.
    pub span: Span,
}

/// What `...` was written as, which changes how the prologue binds the extra arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vararg {
    None,
    /// `...` - the extra arguments stay on the stack and [`super::Op::Vararg`] reads them.
    Anonymous,
    /// `...t` - the extra arguments arrive as a table, bound like a parameter.
    Table,
}

/// Where an upvalue's value comes from, named in the enclosing function's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpvalSource {
    /// A register of the enclosing function.
    ParentLocal(Reg),
    /// An upvalue of the enclosing function, by index.
    ParentUpval(u16),
    /// `_ENV`, which the loader supplies. Only the outermost function has one, at index 0.
    Env,
}

/// Index into [`Program::funcs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncIdx(pub u32);
