//! Expressions, and the one place a name is written to instead of read.

use crate::ast::{ExprId, ExprKind, Field, FuncId, Vararg};
use crate::error::Error;

use super::resolver::Resolver;

impl<'src> Resolver<'_, 'src> {
    /// An assignment target.
    pub(super) fn target(&mut self, id: ExprId) -> Result<(), Error> {
        let expr = self.ast.expr(id);

        match &expr.kind {
            ExprKind::Name(name) => self.access(name, expr.span.start, true),
            _ => self.expr(id),
        }
    }

    pub(super) fn expr(&mut self, id: ExprId) -> Result<(), Error> {
        let expr = self.ast.expr(id);

        match &expr.kind {
            ExprKind::Nil
            | ExprKind::True
            | ExprKind::False
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Str(_)
            | ExprKind::Vararg => {}
            ExprKind::Name(name) => self.access(name, expr.span.start, false)?,
            ExprKind::Paren(inner) => self.expr(*inner)?,
            ExprKind::Index { object, key } => {
                self.expr(*object)?;
                self.expr(*key)?;
            }
            ExprKind::Call { callee, args } => {
                self.expr(*callee)?;
                for &arg in args.iter() {
                    self.expr(arg)?;
                }
            }
            // A method's own name is a key, not a variable
            ExprKind::Method { object, args, .. } => {
                self.expr(*object)?;
                for &arg in args.iter() {
                    self.expr(arg)?;
                }
            }
            ExprKind::Unary { operand, .. } => self.expr(*operand)?,
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left)?;
                self.expr(*right)?;
            }
            ExprKind::Table(fields) => {
                for field in fields.iter() {
                    match field {
                        Field::Positional(value) | Field::Named { value, .. } => {
                            self.expr(*value)?
                        }
                        Field::Keyed { key, value } => {
                            self.expr(*key)?;
                            self.expr(*value)?;
                        }
                    }
                }
            }
            ExprKind::Function(func) => self.func(*func)?,
        }

        Ok(())
    }

    pub(super) fn func(&mut self, id: FuncId) -> Result<(), Error> {
        let ast = self.ast;
        let func = ast.func(id);

        self.enter_function();

        // A method's receiver is not in the parameter list.
        if func.self_var.is_some() {
            self.declare_local(b"self", false);
        }

        for &param in func.params.iter() {
            self.declare_local(param.name, false);
        }

        if let Some(Vararg::Named(var)) = func.vararg {
            self.declare_local(var.name, true);
        }

        self.stats(ast.block(func.body), true)?;
        self.leave_function(func.span.end)
    }
}
