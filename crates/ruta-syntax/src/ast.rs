//! The syntax tree.

use crate::token::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExprId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuncId(u32);

/// A declaration site. Every use of the name resolves to the site's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarId(u32);

/// A name being declared, where no attribute can be written after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Var<'a> {
    pub name: &'a [u8],
    pub id: VarId,
}

#[derive(Debug)]
pub struct Ast<'a> {
    exprs: Vec<Expr<'a>>,
    stats: Vec<Stat<'a>>,
    blocks: Vec<Block>,
    funcs: Vec<Func<'a>>,
    main: BlockId,
}

impl<'a> Ast<'a> {
    /// The chunk's own block.
    pub fn main_block(&self) -> &Block {
        self.block(self.main)
    }

    pub fn block(&self, id: BlockId) -> &Block {
        &self.blocks[id.0 as usize]
    }

    pub fn expr(&self, id: ExprId) -> &Expr<'a> {
        &self.exprs[id.0 as usize]
    }

    pub fn stat(&self, id: StatId) -> &Stat<'a> {
        &self.stats[id.0 as usize]
    }

    pub fn func(&self, id: FuncId) -> &Func<'a> {
        &self.funcs[id.0 as usize]
    }
}

#[derive(Debug)]
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Stat<'a> {
    pub kind: StatKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Block {
    pub stats: Box<[StatId]>,
    pub span: Span,
    pub close_at: u32,
}

#[derive(Debug)]
pub enum ExprKind<'a> {
    Nil,
    True,
    False,
    Int(i64),
    Float(f64),
    Str(Box<[u8]>),
    Vararg,
    Name(&'a [u8]),
    Paren(ExprId),
    Index {
        object: ExprId,
        key: ExprId,
    },
    Call {
        callee: ExprId,
        args: Box<[ExprId]>,
    },
    Method {
        object: ExprId,
        name: &'a [u8],
        args: Box<[ExprId]>,
    },
    Unary {
        op: UnOp,
        operand: ExprId,
    },
    Binary {
        op: BinOp,
        left: ExprId,
        right: ExprId,
    },
    Table(Box<[Field<'a>]>),
    Function(FuncId),
}

#[derive(Debug)]
pub enum Field<'a> {
    Positional(ExprId),
    Named { name: &'a [u8], value: ExprId },
    Keyed { key: ExprId, value: ExprId },
}

#[derive(Debug)]
pub enum StatKind<'a> {
    /// A call standing on its own.
    Expr(ExprId),
    Assign {
        targets: Box<[ExprId]>,
        values: Box<[ExprId]>,
    },
    Local {
        names: Box<[VarName<'a>]>,
        values: Box<[ExprId]>,
    },
    Do(BlockId),
    /// `global <attrib> *` - every name is allowed, with that attribute.
    GlobalAll {
        attribute: Option<Attribute>,
    },
    Global {
        names: Box<[VarName<'a>]>,
        values: Box<[ExprId]>,
    },
    /// `function a.b:c()`. The target is where the value is stored;
    /// whether it was written with a colon lives on the function itself.
    Function {
        target: ExprId,
        func: FuncId,
    },
    GlobalFunction {
        name: &'a [u8],
        func: FuncId,
    },
    LocalFunction {
        name: Var<'a>,
        func: FuncId,
    },
    While {
        condition: ExprId,
        body: BlockId,
    },
    Repeat {
        body: BlockId,
        condition: ExprId,
    },
    NumericFor {
        name: Var<'a>,
        start: ExprId,
        limit: ExprId,
        step: Option<ExprId>,
        body: BlockId,
    },
    GenericFor {
        names: Box<[Var<'a>]>,
        exprs: Box<[ExprId]>,
        body: BlockId,
    },
    /// `if` and each `elseif` are one arm apiece; a trailing `else` block is kept apart.
    If {
        arms: Box<[(ExprId, BlockId)]>,
        otherwise: Option<BlockId>,
    },
    Return(Box<[ExprId]>),
    Break,
    Goto(&'a [u8]),
    Label(&'a [u8]),
}

/// A function body, shared by every form that has one: literals, `function a.b()`, and `local function f()`.
#[derive(Debug)]
pub struct Func<'a> {
    pub params: Box<[Var<'a>]>,
    /// `...`, and the name it was bound to when it had one.
    pub vararg: Option<Vararg<'a>>,
    /// The receiver a method binds before its parameters.
    pub self_var: Option<VarId>,
    pub body: BlockId,
    /// Ends at the `end` that closes the body.
    pub span: Span,
}

/// What the `...` of a parameter list was written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vararg<'a> {
    Anonymous,
    /// `...t` - the extra arguments arrive as a table under this name.
    Named(Var<'a>),
}

/// One name in a `local` or `global` list, with the attributes written after it.
#[derive(Debug)]
pub struct VarName<'a> {
    pub name: &'a [u8],
    pub id: VarId,
    pub attribute: Option<Attribute>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    Const,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Len,
    BNot,
}

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
    And,
    Or,
    BAnd,
    BOr,
    BXor,
    Shl,
    Shr,
}

/// Collects nodes while parsing.
#[derive(Debug, Default)]
pub(crate) struct Builder<'a> {
    exprs: Vec<Expr<'a>>,
    stats: Vec<Stat<'a>>,
    blocks: Vec<Block>,
    funcs: Vec<Func<'a>>,
    vars: u32,
}

impl<'a> Builder<'a> {
    pub(crate) fn expr(&mut self, kind: ExprKind<'a>, span: Span) -> ExprId {
        self.exprs.push(Expr { kind, span });
        ExprId(self.exprs.len() as u32 - 1)
    }

    pub(crate) fn stat(&mut self, kind: StatKind<'a>, span: Span) -> StatId {
        self.stats.push(Stat { kind, span });
        StatId(self.stats.len() as u32 - 1)
    }

    pub(crate) fn block(&mut self, stats: Box<[StatId]>, span: Span, close_at: u32) -> BlockId {
        self.blocks.push(Block {
            stats,
            span,
            close_at,
        });
        BlockId(self.blocks.len() as u32 - 1)
    }

    pub(crate) fn set_block_close(&mut self, id: BlockId, close_at: u32) {
        self.blocks[id.0 as usize].close_at = close_at;
    }

    pub(crate) fn func(&mut self, func: Func<'a>) -> FuncId {
        self.funcs.push(func);
        FuncId(self.funcs.len() as u32 - 1)
    }

    pub(crate) fn var(&mut self) -> VarId {
        self.vars += 1;
        VarId(self.vars - 1)
    }

    /// What a node turned out to be.
    pub(crate) fn kind_of(&self, id: ExprId) -> &ExprKind<'a> {
        &self.exprs[id.0 as usize].kind
    }

    pub(crate) fn finish(self, main: BlockId) -> Ast<'a> {
        Ast {
            exprs: self.exprs,
            stats: self.stats,
            blocks: self.blocks,
            funcs: self.funcs,
            main,
        }
    }
}
