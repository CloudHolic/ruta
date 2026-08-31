//! Statements, and the point at which each one's declarations become visibles.

use crate::ast::{Block, BlockId, StatId, StatKind};
use crate::error::Error;

use super::Resolver;

impl<'src> Resolver<'_, 'src> {
    /// A block's statements without a scope of their own, so that `repeat` can hold its scope open past the last one.
    pub(super) fn stats(&mut self, block: &Block) -> Result<(), Error> {
        for &stat in block.stats.iter() {
            self.stat(stat)?;
        }

        Ok(())
    }

    pub(super) fn block(&mut self, id: BlockId) -> Result<(), Error> {
        let body = self.ast.block(id);
        let height = self.declarations.len();

        self.stats(body)?;
        self.declarations.truncate(height);

        Ok(())
    }

    fn stat(&mut self, id: StatId) -> Result<(), Error> {
        let ast = self.ast;

        match &ast.stat(id).kind {
            StatKind::Expr(expr) => self.expr(*expr)?,
            StatKind::Assign { targets, values } => {
                for &target in targets.iter() {
                    self.target(target)?;
                }

                for &value in values.iter() {
                    self.expr(value)?;
                }
            }
            StatKind::Local { names, values } => {
                // An initializer is read before its own declaration exists.
                // E.g. the `a` of `local a = a` is the outer one.
                for &value in values.iter() {
                    self.expr(value)?;
                }

                for name in names.iter() {
                    self.declare_local(name.name, name.attribute.is_some());
                }
            }
            StatKind::GlobalAll { attribute } => self.declare_global(None, attribute.is_some()),
            StatKind::Global { names, values } => {
                for &value in values.iter() {
                    self.expr(value)?;
                }

                for name in names.iter() {
                    self.declare_global(Some(name.name), name.attribute.is_some());
                }
            }
            StatKind::Function { target, func } => {
                self.target(*target)?;
                self.func(*func)?;
            }
            StatKind::GlobalFunction { name, func } => {
                // A declaration, not an assignment: it shadows a `global<const> *`, and it is
                // one of the declarations that turn off the implicit `global *`.
                self.declare_global(Some(name), false);
                self.func(*func)?;
            }
            StatKind::LocalFunction { name, func } => {
                // Declared before the body so that the function can call itself.
                self.declare_local(name, false);
                self.func(*func)?;
            }
            StatKind::While { condition, body } => {
                self.expr(*condition)?;
                self.block(*body)?;
            }
            StatKind::Repeat { body, condition } => {
                // The body's locals are visible in the condition, so its scope closes after it.
                let body = ast.block(*body);
                let height = self.declarations.len();

                self.stats(body)?;
                self.expr(*condition)?;
                self.declarations.truncate(height);
            }
            StatKind::NumericFor {
                name,
                start,
                limit,
                step,
                body,
            } => {
                self.expr(*start)?;
                self.expr(*limit)?;
                if let Some(step) = step {
                    self.expr(*step)?;
                }

                let height = self.declarations.len();

                self.declare_local(name, true);
                self.block(*body)?;
                self.declarations.truncate(height);
            }
            StatKind::GenericFor { names, exprs, body } => {
                for &expr in exprs.iter() {
                    self.expr(expr)?;
                }

                let height = self.declarations.len();

                for (index, &name) in names.iter().enumerate() {
                    // Only the control variable is read-only.
                    self.declare_local(name, index == 0);
                }

                self.block(*body)?;
                self.declarations.truncate(height);
            }
            StatKind::If { arms, otherwise } => {
                for (condition, body) in arms.iter() {
                    self.expr(*condition)?;
                    self.block(*body)?;
                }

                if let Some(body) = otherwise {
                    self.block(*body)?;
                }
            }
            StatKind::Do(body) => self.block(*body)?,
            StatKind::Return(values) => {
                for &value in values.iter() {
                    self.expr(value)?;
                }
            }
            // `break` names nothing.
            StatKind::Break | StatKind::Goto(_) | StatKind::Label(_) => {}
        }

        Ok(())
    }
}
