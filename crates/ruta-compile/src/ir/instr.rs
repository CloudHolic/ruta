//! Instructions.

use super::block::BlockIdx;
use super::func::FuncIdx;

/// A virtual register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(pub u32);

/// One instruction and the source offset it came from.
#[derive(Debug)]
pub struct Instr {
    pub op: Op,
    pub at: u32,
}

/// A literal the code generator cannot turn into a heap object.
#[derive(Debug)]
pub enum Const {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Box<[u8]>),
}

/// Where a producer's values go.
#[derive(Debug)]
pub enum Results {
    /// Into these registers, padded with nil when fewer arrive.
    Exactly(Box<[Reg]>),
    /// However many there are. The next instruction consumes them.
    Multi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Len,
    BNot,
}

/// The binary operators that are values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
}

#[derive(Debug)]
pub enum Op {
    Const {
        dest: Reg,
        value: Const,
    },
    Move {
        dest: Reg,
        src: Reg,
    },
    GetUpval {
        dest: Reg,
        index: u16,
    },
    SetUpval {
        index: u16,
        src: Reg,
    },
    Closure {
        dest: Reg,
        func: FuncIdx,
    },
    /// Reads `...`. Only valid where the function is [`super::Vararg::Anonymous`].
    Vararg {
        results: Results,
    },
    NewTable {
        dest: Reg,
        array_hint: u32,
        hash_hint: u32,
    },
    Index {
        dest: Reg,
        object: Reg,
        key: Reg,
    },
    SetIndex {
        object: Reg,
        key: Reg,
        src: Reg,
    },
    /// `global name = value`. Unlike [`Op::SetIndex`] this fails when the old value is not
    /// nil, with `global '%s' already defined`. The check is on the *old* value, so it cannot
    /// be folded into an ordinary store. `false` counts as defined; only nil passes.
    DefineGlobal {
        env: Reg,
        key: Reg,
        src: Reg,
    },
    /// The positional fields of a constructor: `table[first ..] = values`, then the pending
    /// multi when `spread`.
    SetList {
        table: Reg,
        first: u32,
        values: Box<[Reg]>,
        spread: bool,
    },
    Unary {
        dest: Reg,
        op: UnOp,
        operand: Reg,
    },
    Binary {
        dest: Reg,
        op: BinOp,
        left: Reg,
        right: Reg,
    },
    Call {
        callee: Reg,
        args: Box<[Reg]>,
        spread: bool,
        results: Results,
    },
    /// Close every upvalue that points at `from` or above, because those slots are dying.
    CloseUpvals {
        from: Reg,
    },
    Jump {
        to: BlockIdx,
    },
    Branch {
        cond: Reg,
        then: BlockIdx,
        otherwise: BlockIdx,
    },
    Return {
        values: Box<[Reg]>,
        spread: bool,
    },
    /// `control`, `limit` and `step` are hidden; `var` is the copy the body sees, and
    /// assigning to it does not affect the iteration.
    ForPrep {
        control: Reg,
        limit: Reg,
        step: Reg,
        var: Reg,
        body: BlockIdx,
        exit: BlockIdx,
    },
    ForLoop {
        control: Reg,
        limit: Reg,
        step: Reg,
        var: Reg,
        body: BlockIdx,
        exit: BlockIdx,
    },
}

impl Op {
    /// Whether this instruction ends a block.
    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Op::Jump { .. }
                | Op::Branch { .. }
                | Op::Return { .. }
                | Op::ForPrep { .. }
                | Op::ForLoop { .. }
        )
    }
}
