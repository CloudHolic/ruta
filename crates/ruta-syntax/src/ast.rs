//! The syntax tree.

use crate::token::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExprId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockId(u32);

#[derive(Debug)]
pub struct Ast<'a> {
    exprs: Vec<Expr<'a>>,
    stats: Vec<Stat>,
    blocks: Vec<Block>,
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

    pub fn stat(&self, id: StatId) -> &Stat {
        &self.stats[id.0 as usize]
    }
}

#[derive(Debug)]
pub struct Expr<'a> {
    pub kind: ExprKind<'a>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Stat {
    pub kind: StatKind,
    pub span: Span,
}

#[derive(Debug)]
pub struct Block {
    pub stats: Box<[StatId]>,
    pub span: Span,
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
}

#[derive(Debug)]
pub enum Field<'a> {
    Positional(ExprId),
    Named { name: &'a [u8], value: ExprId },
    Keyed { key: ExprId, value: ExprId },
}

#[derive(Debug)]
pub enum StatKind {
    Return(Box<[ExprId]>),
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
    stats: Vec<Stat>,
    blocks: Vec<Block>,
}

impl<'a> Builder<'a> {
    pub(crate) fn expr(&mut self, kind: ExprKind<'a>, span: Span) -> ExprId {
        self.exprs.push(Expr { kind, span });
        ExprId(self.exprs.len() as u32 - 1)
    }

    pub(crate) fn stat(&mut self, kind: StatKind, span: Span) -> StatId {
        self.stats.push(Stat { kind, span });
        StatId(self.stats.len() as u32 - 1)
    }

    pub(crate) fn block(&mut self, stats: Box<[StatId]>, span: Span) -> BlockId {
        self.blocks.push(Block { stats, span });
        BlockId(self.blocks.len() as u32 - 1)
    }

    pub(crate) fn finish(self, main: BlockId) -> Ast<'a> {
        Ast {
            exprs: self.exprs,
            stats: self.stats,
            blocks: self.blocks,
            main,
        }
    }
}
