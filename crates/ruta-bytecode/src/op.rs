//! Instructions, and how they sit in the byte stream.

/// A count operand meaning "as many as there are" rather than a fixed number.
/// Register indices never reach this value: a frame holds at most 255 registers.
pub const MULTI: u8 = 255;

const ALL: [OpCode; 48] = [
    OpCode::LoadNil,
    OpCode::LoadTrue,
    OpCode::LoadFalse,
    OpCode::LoadConst,
    OpCode::LoadConstWide,
    OpCode::Move,
    OpCode::GetUpval,
    OpCode::SetUpval,
    OpCode::CloseUpvals,
    OpCode::Closure,
    OpCode::Vararg,
    OpCode::NewTable,
    OpCode::Index,
    OpCode::SetIndex,
    OpCode::DefineGlobal,
    OpCode::SetList,
    OpCode::SetListSpread,
    OpCode::Neg,
    OpCode::Not,
    OpCode::Len,
    OpCode::BNot,
    OpCode::Add,
    OpCode::Sub,
    OpCode::Mul,
    OpCode::Div,
    OpCode::IDiv,
    OpCode::Mod,
    OpCode::Pow,
    OpCode::Concat,
    OpCode::Eq,
    OpCode::Ne,
    OpCode::Lt,
    OpCode::Le,
    OpCode::Gt,
    OpCode::Ge,
    OpCode::BAnd,
    OpCode::BOr,
    OpCode::BXor,
    OpCode::Shl,
    OpCode::Shr,
    OpCode::Call,
    OpCode::TailCall,
    OpCode::Return,
    OpCode::Jump,
    OpCode::JumpIfTrue,
    OpCode::JumpIfFalse,
    OpCode::ForPrep,
    OpCode::ForLoop,
];

/// What one operand of an instruction occupies in the byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    /// A register index, or a count bounded by the register limit.
    U8,
    /// A constant pool index, narrow form.
    U16,
    /// An index that is not bounded by the register limit.
    U32,
    /// A jump displacement, counted from the byte after the instruction.
    I32,
}

impl Operand {
    pub fn width(self) -> u32 {
        match self {
            Operand::U8 => 1,
            Operand::U16 => 2,
            Operand::U32 | Operand::I32 => 4,
        }
    }
}

/// The byte an instruction begins with. The value written to the stream is the discriminant,
/// so these are spelled out rather than left to declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    LoadNil = 0,
    LoadTrue = 1,
    LoadFalse = 2,
    LoadConst = 3,
    LoadConstWide = 4,
    Move = 5,

    GetUpval = 6,
    SetUpval = 7,
    CloseUpvals = 8,
    Closure = 9,
    Vararg = 10,

    NewTable = 11,
    Index = 12,
    SetIndex = 13,
    DefineGlobal = 14,
    SetList = 15,
    SetListSpread = 16,

    Neg = 17,
    Not = 18,
    Len = 19,
    BNot = 20,

    Add = 21,
    Sub = 22,
    Mul = 23,
    Div = 24,
    IDiv = 25,
    Mod = 26,
    Pow = 27,
    Concat = 28,
    Eq = 29,
    Ne = 30,
    Lt = 31,
    Le = 32,
    Gt = 33,
    Ge = 34,
    BAnd = 35,
    BOr = 36,
    BXor = 37,
    Shl = 38,
    Shr = 39,

    Call = 40,
    TailCall = 41,
    Return = 42,

    Jump = 43,
    JumpIfTrue = 44,
    JumpIfFalse = 45,
    ForPrep = 46,
    ForLoop = 47,
}

impl OpCode {
    pub fn from_byte(byte: u8) -> Option<OpCode> {
        if byte <= OpCode::ForLoop as u8 {
            Some(ALL[byte as usize])
        } else {
            None
        }
    }

    /// The operands this instruction carries, in the order they are written.
    pub fn operands(self) -> &'static [Operand] {
        use Operand::{I32, U8, U16, U32};

        match self {
            OpCode::LoadNil | OpCode::LoadTrue | OpCode::LoadFalse | OpCode::CloseUpvals => &[U8],
            OpCode::LoadConst => &[U8, U16],
            OpCode::LoadConstWide => &[U8, U32],
            OpCode::Move
            | OpCode::GetUpval
            | OpCode::SetUpval
            | OpCode::Vararg
            | OpCode::Return
            | OpCode::TailCall
            | OpCode::Neg
            | OpCode::Not
            | OpCode::Len
            | OpCode::BNot => &[U8, U8],
            OpCode::Closure => &[U8, U32],
            OpCode::NewTable => &[U8, U32, U32],
            OpCode::Index
            | OpCode::SetIndex
            | OpCode::DefineGlobal
            | OpCode::Call
            | OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::IDiv
            | OpCode::Mod
            | OpCode::Pow
            | OpCode::Concat
            | OpCode::Eq
            | OpCode::Ne
            | OpCode::Lt
            | OpCode::Le
            | OpCode::Gt
            | OpCode::Ge
            | OpCode::BAnd
            | OpCode::BOr
            | OpCode::BXor
            | OpCode::Shl
            | OpCode::Shr => &[U8, U8, U8],
            OpCode::SetList => &[U8, U8, U8, U32],
            OpCode::SetListSpread => &[U8, U8, U32],
            OpCode::Jump => &[I32],
            OpCode::JumpIfTrue | OpCode::JumpIfFalse | OpCode::ForPrep | OpCode::ForLoop => {
                &[U8, I32]
            }
        }
    }
}

/// One decoded instruction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    LoadNil {
        dest: u8,
    },
    LoadTrue {
        dest: u8,
    },
    LoadFalse {
        dest: u8,
    },
    /// `constant` indexes the prototype's own pool.
    LoadConst {
        dest: u8,
        constant: u32,
    },
    Move {
        dest: u8,
        src: u8,
    },

    GetUpval {
        dest: u8,
        index: u8,
    },
    SetUpval {
        index: u8,
        src: u8,
    },
    /// Closes every upvalue pointing at `from` or above.
    CloseUpvals {
        from: u8,
    },
    /// `child` indexes the enclosing prototype's own children.
    Closure {
        dest: u8,
        child: u32,
    },
    /// Reads `...` into `first ..`. `count` is [`MULTI`] when the values run to the top of
    /// the frame.
    Vararg {
        first: u8,
        count: u8,
    },

    NewTable {
        dest: u8,
        array_hint: u32,
        hash_hint: u32,
    },
    Index {
        dest: u8,
        object: u8,
        key: u8,
    },
    SetIndex {
        object: u8,
        key: u8,
        src: u8,
    },
    /// Unlike [`Op::SetIndex`] this fails when the old value is not nil, with
    /// `global '%s' already defined`. `false` counts as defined; only nil passes.
    DefineGlobal {
        env: u8,
        key: u8,
        src: u8,
    },
    /// `table[first_index ..] = first .. first + count`.
    SetList {
        table: u8,
        first: u8,
        count: u8,
        first_index: u32,
    },
    /// The same, with the values running to the top of the frame.
    SetListSpread {
        table: u8,
        first: u8,
        first_index: u32,
    },
    Neg {
        dest: u8,
        operand: u8,
    },
    Not {
        dest: u8,
        operand: u8,
    },
    Len {
        dest: u8,
        operand: u8,
    },
    BNot {
        dest: u8,
        operand: u8,
    },
    Add {
        dest: u8,
        left: u8,
        right: u8,
    },
    Sub {
        dest: u8,
        left: u8,
        right: u8,
    },
    Mul {
        dest: u8,
        left: u8,
        right: u8,
    },
    Div {
        dest: u8,
        left: u8,
        right: u8,
    },
    IDiv {
        dest: u8,
        left: u8,
        right: u8,
    },
    Mod {
        dest: u8,
        left: u8,
        right: u8,
    },
    Pow {
        dest: u8,
        left: u8,
        right: u8,
    },
    Concat {
        dest: u8,
        left: u8,
        right: u8,
    },
    Eq {
        dest: u8,
        left: u8,
        right: u8,
    },
    Ne {
        dest: u8,
        left: u8,
        right: u8,
    },
    Lt {
        dest: u8,
        left: u8,
        right: u8,
    },
    Le {
        dest: u8,
        left: u8,
        right: u8,
    },
    Gt {
        dest: u8,
        left: u8,
        right: u8,
    },
    Ge {
        dest: u8,
        left: u8,
        right: u8,
    },
    BAnd {
        dest: u8,
        left: u8,
        right: u8,
    },
    BOr {
        dest: u8,
        left: u8,
        right: u8,
    },
    BXor {
        dest: u8,
        left: u8,
        right: u8,
    },
    Shl {
        dest: u8,
        left: u8,
        right: u8,
    },
    Shr {
        dest: u8,
        left: u8,
        right: u8,
    },
    /// The arguments sit at `callee + 1 ..`, the results land at `callee ..`. Either count
    /// is [`MULTI`].
    Call {
        callee: u8,
        args: u8,
        results: u8,
    },
    TailCall {
        callee: u8,
        args: u8,
    },
    Return {
        first: u8,
        count: u8,
    },
    Jump {
        offset: i32,
    },
    JumpIfTrue {
        cond: u8,
        offset: i32,
    },
    JumpIfFalse {
        cond: u8,
        offset: i32,
    },
    /// `control`, `control + 1`, `control + 2` and `control + 3` are the counter, the limit,
    /// the step and the copy the body sees. Assigning to the copy does not affect the
    /// iteration.
    ForPrep {
        control: u8,
        offset: i32,
    },
    ForLoop {
        control: u8,
        offset: i32,
    },
}

impl Op {
    pub fn opcode(&self) -> OpCode {
        match self {
            Op::LoadNil { .. } => OpCode::LoadNil,
            Op::LoadTrue { .. } => OpCode::LoadTrue,
            Op::LoadFalse { .. } => OpCode::LoadFalse,
            Op::LoadConst { .. } => OpCode::LoadConst,
            Op::Move { .. } => OpCode::Move,
            Op::GetUpval { .. } => OpCode::GetUpval,
            Op::SetUpval { .. } => OpCode::SetUpval,
            Op::CloseUpvals { .. } => OpCode::CloseUpvals,
            Op::Closure { .. } => OpCode::Closure,
            Op::Vararg { .. } => OpCode::Vararg,
            Op::NewTable { .. } => OpCode::NewTable,
            Op::Index { .. } => OpCode::Index,
            Op::SetIndex { .. } => OpCode::SetIndex,
            Op::DefineGlobal { .. } => OpCode::DefineGlobal,
            Op::SetList { .. } => OpCode::SetList,
            Op::SetListSpread { .. } => OpCode::SetListSpread,
            Op::Neg { .. } => OpCode::Neg,
            Op::Not { .. } => OpCode::Not,
            Op::Len { .. } => OpCode::Len,
            Op::BNot { .. } => OpCode::BNot,
            Op::Add { .. } => OpCode::Add,
            Op::Sub { .. } => OpCode::Sub,
            Op::Mul { .. } => OpCode::Mul,
            Op::Div { .. } => OpCode::Div,
            Op::IDiv { .. } => OpCode::IDiv,
            Op::Mod { .. } => OpCode::Mod,
            Op::Pow { .. } => OpCode::Pow,
            Op::Concat { .. } => OpCode::Concat,
            Op::Eq { .. } => OpCode::Eq,
            Op::Ne { .. } => OpCode::Ne,
            Op::Lt { .. } => OpCode::Lt,
            Op::Le { .. } => OpCode::Le,
            Op::Gt { .. } => OpCode::Gt,
            Op::Ge { .. } => OpCode::Ge,
            Op::BAnd { .. } => OpCode::BAnd,
            Op::BOr { .. } => OpCode::BOr,
            Op::BXor { .. } => OpCode::BXor,
            Op::Shl { .. } => OpCode::Shl,
            Op::Shr { .. } => OpCode::Shr,
            Op::Call { .. } => OpCode::Call,
            Op::TailCall { .. } => OpCode::TailCall,
            Op::Return { .. } => OpCode::Return,
            Op::Jump { .. } => OpCode::Jump,
            Op::JumpIfTrue { .. } => OpCode::JumpIfTrue,
            Op::JumpIfFalse { .. } => OpCode::JumpIfFalse,
            Op::ForPrep { .. } => OpCode::ForPrep,
            Op::ForLoop { .. } => OpCode::ForLoop,
        }
    }
}

/// How many bytes the instruction beginning with `byte` occupies, the opcode included.
pub fn instruction_len(byte: u8) -> Option<u32> {
    let opcode = OpCode::from_byte(byte)?;
    let operands: u32 = opcode
        .operands()
        .iter()
        .map(|operand| operand.width())
        .sum();

    Some(1 + operands)
}
