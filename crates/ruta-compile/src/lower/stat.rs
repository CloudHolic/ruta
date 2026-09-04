//! Statements, and the scope a block gives its locals.

use ruta_syntax::ast::{Block, BlockId, StatId, StatKind};

use crate::ir::{Op, Reg, Results};

use super::func::Lowerer;

impl Lowerer<'_> {
    pub(super) fn stats(&mut self, block: &Block) {
        for stat in block.stats.iter().copied() {
            // A statement after a jump is unreachable, and still has to go somewhere.
            if self.is_terminated() {
                let next = self.new_block();
                self.switch_to(next);
            }

            self.stat(stat);
        }
    }

    fn body(&mut self, id: BlockId) {
        let ast = self.ast;
        let depth = self.state().vars.len();

        self.stats(ast.block(id));
        self.state().vars.truncate(depth);
    }

    fn stat(&mut self, id: StatId) {
        let ast = self.ast;
        let stat = ast.stat(id);
        let at = stat.span.start;

        match &stat.kind {
            StatKind::Local { names, values } => {
                let dests: Vec<Reg> = names.iter().map(|_| self.reg()).collect();
                self.explist(values, &dests, at);

                for (name, dest) in names.iter().zip(dests) {
                    self.state().vars.push((name.id, dest));
                }
            }
            StatKind::Do(body) => self.body(*body),
            StatKind::Expr(value) => self.multi(*value, Results::Exactly(Box::new([]))),
            StatKind::Return(values) => {
                let (values, spread) = self.explist_open(values);
                self.emit(Op::Return { values, spread }, at);
            }
            kind => unimplemented!("{kind:?}"),
        }
    }
}
