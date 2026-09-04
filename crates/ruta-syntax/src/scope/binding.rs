//! What each name turned out to name.

use crate::ast::{ExprId, FuncId, StatId, VarId};

/// Where a value lives, relative to the function that reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// A local of that function, declared at this site.
    Local(VarId),
    /// An upvalue of that function, by index into its capture list.
    Upvalue(u16),
}

/// What one written name resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// The name is that variable.
    Variable(Access),
    /// The name is a field of `_ENV`, which is itself reached this way.
    Global(Access),
}

/// Where one upvalue's value comes from, named in the enclosing function's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
    ParentLocal(VarId),
    ParentUpvalue(u16),
    /// `_ENV`, which the loader supplies. Only the outermost function has one.
    Env,
}

#[derive(Debug, Default)]
pub struct FunctionBindings {
    /// One entry per upvalue, in the order the body first referred to it.
    pub upvalues: Box<[Capture]>,
}

/// Every name in a chunk, answered.
#[derive(Debug)]
pub struct Bindings {
    uses: Box<[Option<Binding>]>,
    jumps: Box<[Option<StatId>]>,
    funcs: Box<[FunctionBindings]>,
}

impl Bindings {
    /// What the name written at this expression resolves to, or `None` when the expression is not a name.
    pub fn at(&self, id: ExprId) -> Option<Binding> {
        self.uses[id.index()]
    }

    /// The label statement this goto reaches, or `None` when the statement is not a goto.
    pub fn target(&self, goto: StatId) -> Option<StatId> {
        self.jumps[goto.index()]
    }

    /// The outermost function, the one a chunk itself is.
    pub fn main(&self) -> &FunctionBindings {
        &self.funcs[0]
    }

    pub fn function(&self, id: FuncId) -> &FunctionBindings {
        &self.funcs[id.index() + 1]
    }

    pub(super) fn new(exprs: usize, stats: usize, funcs: usize) -> Bindings {
        Bindings {
            uses: vec![None; exprs].into_boxed_slice(),
            jumps: vec![None; stats].into_boxed_slice(),
            funcs: (0..funcs).map(|_| FunctionBindings::default()).collect(),
        }
    }

    pub(super) fn record(&mut self, id: ExprId, binding: Binding) {
        self.uses[id.index()] = Some(binding);
    }

    pub(super) fn record_jump(&mut self, goto: StatId, label: StatId) {
        self.jumps[goto.index()] = Some(label);
    }

    pub(super) fn set_upvalues(&mut self, index: usize, upvalues: Box<[Capture]>) {
        self.funcs[index].upvalues = upvalues;
    }
}
