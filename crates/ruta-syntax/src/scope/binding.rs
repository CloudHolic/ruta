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

/// One upvalue: the name the body first asked for it by, and where its value comes from.
#[derive(Debug)]
pub struct Upvalue {
    pub name: Box<[u8]>,
    pub capture: Capture,
}

#[derive(Debug, Default)]
pub struct FunctionBindings {
    /// One entry per upvalue, in the order the body first referred to it.
    pub upvalues: Box<[Upvalue]>,
}

/// Every name in a chunk, answered.
#[derive(Debug)]
pub struct Bindings {
    uses: Box<[Option<Binding>]>,
    jumps: Box<[Option<StatId>]>,
    envs: Box<[Option<Access>]>,
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

    /// How the function reaches `_ENV` where this statement assigns a global.
    /// A `global` declaration without an initializer assigns nothing and needs none.
    pub fn env(&self, stat: StatId) -> Option<Access> {
        self.envs[stat.index()]
    }

    /// The outermost function, the one a chunk itself is.
    pub fn main(&self) -> &FunctionBindings {
        &self.funcs[0]
    }

    pub fn function(&self, id: FuncId) -> &FunctionBindings {
        &self.funcs[id.index() + 1]
    }

    /// Every local that some function captures. A declaration site belongs to one function,
    /// so an id here is captured out of the function that declares it.
    pub fn captured(&self) -> impl Iterator<Item = VarId> {
        self.funcs
            .iter()
            .flat_map(|func| func.upvalues.iter())
            .filter_map(|upvalue| match upvalue.capture {
                Capture::ParentLocal(var) => Some(var),
                Capture::ParentUpvalue(_) | Capture::Env => None,
            })
    }

    pub(super) fn new(exprs: usize, stats: usize, funcs: usize) -> Bindings {
        Bindings {
            uses: vec![None; exprs].into_boxed_slice(),
            jumps: vec![None; stats].into_boxed_slice(),
            envs: vec![None; stats].into_boxed_slice(),
            funcs: (0..funcs).map(|_| FunctionBindings::default()).collect(),
        }
    }

    pub(super) fn record(&mut self, id: ExprId, binding: Binding) {
        self.uses[id.index()] = Some(binding);
    }

    pub(super) fn record_jump(&mut self, goto: StatId, label: StatId) {
        self.jumps[goto.index()] = Some(label);
    }

    pub(super) fn record_env(&mut self, stat: StatId, access: Access) {
        self.envs[stat.index()] = Some(access);
    }

    pub(super) fn set_upvalues(&mut self, index: usize, upvalues: Box<[Upvalue]>) {
        self.funcs[index].upvalues = upvalues;
    }
}
