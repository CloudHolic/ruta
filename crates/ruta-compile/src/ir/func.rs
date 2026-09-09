//! Functions, and where each one's captured values come from.

use ruta_syntax::token::Span;

use super::block::{Block, BlockIdx};
use super::instr::Reg;

/// A point in a function's instruction stream. The emitter turns these into a pc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub block: BlockIdx,
    pub instr: u32,
}

/// A register a declaration holds for the whole of its scope.
/// Scopes nest, so `number` never collides with another slot that is in scope at the time.
#[derive(Debug)]
pub struct Slot {
    /// Absent where the source writes no name: a numeric `for` holds three such registers,
    /// and so does the value a generic `for` closes.
    pub name: Option<Box<[u8]>>,
    pub reg: Reg,
    /// How deep in the scope stack it sits, counted in slots.
    pub number: u32,
    pub enters: Position,
    pub leaves: Position,
}

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
    pub upvalues: Vec<Upval>,
    /// One entry per declaration, in the order the body declares them.
    pub slots: Vec<Slot>,
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

/// One upvalue, as the prototype will carry it. The source is what allocation rewrites;
/// the name is only ever read.
#[derive(Debug)]
pub struct Upval {
    pub name: Box<[u8]>,
    pub source: UpvalSource,
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
