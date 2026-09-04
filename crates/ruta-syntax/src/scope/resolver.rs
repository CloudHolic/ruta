//! The resolver's state: the declaration stack, and what a name turns out to be.

use crate::ast::{Ast, ExprId};
use crate::error::{Error, ErrorKind, Near};

use super::binding::{Access, Binding, Bindings, Capture};
use super::label::{BlockFrame, FunctionFrame};

#[derive(Debug)]
pub(super) struct Resolver<'a, 'src> {
    pub(super) ast: &'a Ast<'src>,
    pub(super) declarations: Vec<Declaration<'src>>,
    pub(super) blocks: Vec<BlockFrame<'src>>,
    pub(super) functions: Vec<FunctionFrame<'src>>,
    pub(super) bindings: Bindings,
}

#[derive(Debug)]
pub(super) struct Declaration<'src> {
    /// `None` for the wildcard `global *`.
    pub(super) name: Option<&'src [u8]>,
    /// How the function that declared it reaches it. `None` for a global.
    at_owner: Option<Access>,
    /// The depth of the function frame that declared it.
    function: usize,
    is_global: bool,
    readonly: bool,
}

/// What a name turned out to be, before the capture chain is walked.
#[derive(Debug, Clone, Copy)]
enum Resolution {
    Variable {
        at_owner: Access,
        owner: usize,
        readonly: bool,
    },
    Global {
        readonly: bool,
    },
    Undeclared,
}

impl<'src> Resolver<'_, 'src> {
    pub(super) fn declare_local(&mut self, name: &'src [u8], at: Access, readonly: bool) {
        self.declarations.push(Declaration {
            name: Some(name),
            at_owner: Some(at),
            function: self.functions.len() - 1,
            is_global: false,
            readonly,
        })
    }

    /// Globals take `None` for the wildcard `global *`.
    pub(super) fn declare_global(&mut self, name: Option<&'src [u8]>, readonly: bool) {
        self.declarations.push(Declaration {
            name,
            at_owner: None,
            function: self.functions.len() - 1,
            is_global: true,
            readonly,
        })
    }

    /// One access and the three ways it is refused, in the order the reference refuses them.
    pub(super) fn access(
        &mut self,
        id: ExprId,
        name: &'src [u8],
        at: u32,
        writing: bool,
    ) -> Result<(), Error> {
        let (binding, readonly) = match self.lookup(name) {
            Resolution::Undeclared => {
                return Err(error(ErrorKind::VariableNotDeclared(name.into()), at));
            }
            Resolution::Global { readonly } => {
                let Resolution::Variable {
                    at_owner, owner, ..
                } = self.lookup(b"_ENV")
                else {
                    return Err(error(ErrorKind::EnvIsGlobal(name.into()), at));
                };

                (
                    Binding::Global(self.reach(b"_ENV", owner, at_owner)),
                    readonly,
                )
            }
            Resolution::Variable {
                at_owner,
                owner,
                readonly,
            } => (
                Binding::Variable(self.reach(name, owner, at_owner)),
                readonly,
            ),
        };

        if writing && readonly {
            return Err(error(ErrorKind::ConstAssignment(name.into()), at));
        }

        self.bindings.record(id, binding);

        Ok(())
    }

    fn lookup(&self, name: &[u8]) -> Resolution {
        let mut wildcard = None;
        let mut any_global = false;

        for declaration in self.declarations.iter().rev() {
            match declaration.name {
                Some(declared) if declared == name => {
                    let readonly = declaration.readonly;

                    return match declaration.at_owner {
                        Some(at_owner) => Resolution::Variable {
                            at_owner,
                            owner: declaration.function,
                            readonly,
                        },
                        None => Resolution::Global { readonly },
                    };
                }
                Some(_) => any_global |= declaration.is_global,
                None => {
                    any_global = true;
                    wildcard = wildcard.or(Some(declaration.readonly));
                }
            }
        }

        match wildcard {
            Some(readonly) => Resolution::Global { readonly },
            None if any_global => Resolution::Undeclared,
            None => Resolution::Global { readonly: false },
        }
    }

    /// Makes a variable owned by an enclosing function reachable from the innermost one,
    /// capturing it through every function in between.
    fn reach(&mut self, name: &'src [u8], owner: usize, at_owner: Access) -> Access {
        let mut access = at_owner;

        for depth in (owner + 1)..self.functions.len() {
            let capture = match access {
                Access::Local(var) => Capture::ParentLocal(var),
                Access::Upvalue(index) => Capture::ParentUpvalue(index),
            };

            access = Access::Upvalue(self.capture(depth, name, capture));
        }

        access
    }

    /// Where that function keeps `name`, adding it to the capture list when it is not there.
    fn capture(&mut self, depth: usize, name: &'src [u8], capture: Capture) -> u16 {
        let frame = &mut self.functions[depth];

        if let Some(index) = frame.upvalues.iter().position(|(known, _)| *known == name) {
            return index as u16;
        }

        frame.upvalues.push((name, capture));

        (frame.upvalues.len() - 1) as u16
    }
}

/// Walks a parsed chunk, refusing the first name it cannot justify.
pub fn resolve(ast: &Ast<'_>) -> Result<Bindings, Error> {
    let mut resolver = Resolver {
        ast,
        declarations: vec![Declaration {
            name: Some(b"_ENV"),
            at_owner: Some(Access::Upvalue(0)),
            function: 0,
            is_global: false,
            readonly: false,
        }],
        blocks: Vec::new(),
        functions: Vec::new(),
        bindings: Bindings::new(ast.expr_count(), ast.func_count() + 1),
    };

    let main = ast.main_block();

    // The chunk is a function, which is why a goto at its top level is refused at its end.
    resolver.enter_function(0);
    resolver.functions[0].upvalues.push((b"_ENV", Capture::Env));

    resolver.stats(main, true)?;
    resolver.leave_function(main.close_at)?;

    Ok(resolver.bindings)
}

/// Scope errors never carry a `near` clause.
pub(super) fn error(kind: ErrorKind, at: u32) -> Error {
    Error {
        kind,
        at,
        near: Near::None,
    }
}
