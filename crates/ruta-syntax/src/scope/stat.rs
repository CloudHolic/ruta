//! Statements, the point at which each one's declarations become visible, and where labels sit.

use crate::ast::{Block, BlockId, StatId, StatKind};
use crate::error::Error;

use super::label::{Exit, Goto};
use super::resolver::Resolver;

impl<'src> Resolver<'_, 'src> {
    /// A block's statements without a scope of their own, so that `repeat` can hold its scope open past the last one.
    pub(super) fn stats(&mut self, block: &Block, tail_labels: bool) -> Result<(), Error> {
        let ast = self.ast;
        let stats = &block.stats;
        let mut index = 0;

        while index < stats.len() {
            if !matches!(ast.stat(stats[index]).kind, StatKind::Label(_)) {
                self.stat(stats[index])?;
                index += 1;
                continue;
            }

            // Labels written back to back are registered last one first, which is what decides
            // the line a collision between them names.
            let mut last = index;
            while last + 1 < stats.len()
                && matches!(ast.stat(stats[last + 1]).kind, StatKind::Label(_))
            {
                last += 1;
            }

            let report_at = ast.stat(stats[last]).span.end;
            let tail = tail_labels && last + 1 == stats.len();

            for &id in stats[index..=last].iter().rev() {
                let stat = ast.stat(id);
                let StatKind::Label(name) = stat.kind else {
                    unreachable!("the run holds labels only")
                };

                self.declare_label(name, stat.span.start, report_at, tail)?;
            }

            index = last + 1;
        }

        Ok(())
    }

    pub(super) fn block(&mut self, id: BlockId) -> Result<(), Error> {
        let body = self.ast.block(id);

        self.enter_block();
        self.stats(body, true)?;
        self.leave_block(body.close_at, Exit::Block)
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

                self.enter_block();
                self.stats(body, false)?;
                self.expr(*condition)?;
                self.leave_block(body.close_at, Exit::Block)?;
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

                self.enter_block();
                self.declare_local(name, true);
                self.block(*body)?;
                self.leave_block(ast.block(*body).close_at, Exit::Block)?;
            }
            StatKind::GenericFor { names, exprs, body } => {
                for &expr in exprs.iter() {
                    self.expr(expr)?;
                }

                self.enter_block();

                for (index, &name) in names.iter().enumerate() {
                    // Only the control variable is read-only.
                    self.declare_local(name, index == 0);
                }

                self.block(*body)?;
                self.leave_block(ast.block(*body).close_at, Exit::Block)?;
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
            StatKind::Goto(name) => {
                let goto = Goto {
                    name,
                    at: ast.stat(id).span.start,
                    declarations: self.declarations.len(),
                };

                self.blocks
                    .last_mut()
                    .expect("inside a block")
                    .gotos
                    .push(goto)
            }
            StatKind::Break => {}
            StatKind::Label(_) => unreachable!("Labels are registered by stats()"),
        }

        Ok(())
    }
}
