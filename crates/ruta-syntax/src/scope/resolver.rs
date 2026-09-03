//! The resolver's state: the declaration stack, and what a name turns out to be.

use crate::ast::Ast;
use crate::error::{Error, ErrorKind, Near};

use super::label::BlockFrame;

#[derive(Debug)]
pub(super) struct Resolver<'a, 'src> {
    pub(super) ast: &'a Ast<'src>,
    pub(super) declarations: Vec<Declaration<'src>>,
    pub(super) blocks: Vec<BlockFrame<'src>>,
    /// Where each function's block start.
    pub(super) functions: Vec<usize>,
}

#[derive(Debug)]
pub(super) struct Declaration<'src> {
    /// `None` for the wildcard `global *`.
    pub(super) name: Option<&'src [u8]>,
    is_global: bool,
    readonly: bool,
}

/// What a name turned out to be.
#[derive(Debug, Clone, Copy)]
enum Resolution {
    Local { readonly: bool },
    Global { readonly: bool },
    Undeclared,
}

impl<'src> Resolver<'_, 'src> {
    pub(super) fn declare_local(&mut self, name: &'src [u8], readonly: bool) {
        self.declarations.push(Declaration {
            name: Some(name),
            is_global: false,
            readonly,
        })
    }

    /// Globals take `None` for the wildcard `global *`.
    pub(super) fn declare_global(&mut self, name: Option<&'src [u8]>, readonly: bool) {
        self.declarations.push(Declaration {
            name,
            is_global: true,
            readonly,
        })
    }

    /// One access and the three ways it is refused, in the order the reference refuses them.
    pub(super) fn access(&self, name: &[u8], at: u32, writing: bool) -> Result<(), Error> {
        let readonly = match self.lookup(name) {
            Resolution::Undeclared => {
                return Err(error(ErrorKind::VariableNotDeclared(name.into()), at));
            }
            Resolution::Global { readonly } => {
                if matches!(self.lookup(b"_ENV"), Resolution::Global { .. }) {
                    return Err(error(ErrorKind::EnvIsGlobal(name.into()), at));
                }

                readonly
            }
            Resolution::Local { readonly } => readonly,
        };

        if writing && readonly {
            return Err(error(ErrorKind::ConstAssignment(name.into()), at));
        }

        Ok(())
    }

    fn lookup(&self, name: &[u8]) -> Resolution {
        let mut wildcard = None;
        let mut any_global = false;

        for declaration in self.declarations.iter().rev() {
            match declaration.name {
                Some(declared) if declared == name => {
                    let readonly = declaration.readonly;

                    return if declaration.is_global {
                        Resolution::Global { readonly }
                    } else {
                        Resolution::Local { readonly }
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
}

/// Walks a parsed chunk, refusing the first name it cannot justify.
pub fn resolve(ast: &Ast<'_>) -> Result<(), Error> {
    let mut resolver = Resolver {
        ast,
        declarations: vec![Declaration {
            name: Some(b"_ENV"),
            is_global: false,
            readonly: false,
        }],
        blocks: Vec::new(),
        functions: Vec::new(),
    };

    let main = ast.main_block();

    // The chunk is a function, which is why a goto at its top level is refused at its end.
    resolver.enter_function();
    resolver.stats(main, true)?;
    resolver.leave_function(main.close_at)
}

/// Scope errors never carry a `near` clause.
pub(super) fn error(kind: ErrorKind, at: u32) -> Error {
    Error {
        kind,
        at,
        near: Near::None,
    }
}
