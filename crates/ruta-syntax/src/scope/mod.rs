//! Name resolution, and the assignments it refuses.

mod expr;
mod label;
mod stat;

use crate::ast::Ast;
use crate::error::{Error, ErrorKind, Near};
use label::BlockFrame;

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

#[derive(Debug)]
struct Resolver<'a, 'src> {
    ast: &'a Ast<'src>,
    declarations: Vec<Declaration<'src>>,
    blocks: Vec<BlockFrame<'src>>,
    /// Where each function's block start.
    functions: Vec<usize>,
}

#[derive(Debug)]
struct Declaration<'src> {
    /// `None` for the wildcard `global *`.
    name: Option<&'src [u8]>,
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
    fn declare_local(&mut self, name: &'src [u8], readonly: bool) {
        self.declarations.push(Declaration {
            name: Some(name),
            is_global: false,
            readonly,
        })
    }

    /// Globals take `None` for the wildcard `global *`.
    fn declare_global(&mut self, name: Option<&'src [u8]>, readonly: bool) {
        self.declarations.push(Declaration {
            name,
            is_global: true,
            readonly,
        })
    }

    /// One access and the three ways it is refused, in the order the reference refuses them.
    fn access(&self, name: &[u8], at: u32, writing: bool) -> Result<(), Error> {
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

/// Scope errors never carry a `near` clause.
fn error(kind: ErrorKind, at: u32) -> Error {
    Error {
        kind,
        at,
        near: Near::None,
    }
}
