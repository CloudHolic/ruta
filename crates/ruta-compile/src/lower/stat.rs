//! Statements, and the scope a block gives its locals.

use ruta_syntax::ast::{Attribute, Block, BlockId, ExprId, ExprKind, StatId, StatKind};
use ruta_syntax::scope::{Access, Binding};

use crate::ir::{BinOp, Const, Op, Reg, Results};

use super::func::Lowerer;

/// Where an assignment puts its value, worked out before any value is evaluated.
#[derive(Debug, Clone, Copy)]
enum Target {
    Local(Reg),
    Upvalue(u16),
    Index { object: Reg, key: Reg },
}

impl Lowerer<'_> {
    /// A block's statements without a scope of their own.
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
        let block = ast.block(id);
        let depth = self.state().vars.len();

        self.stats(block);
        self.close_upvals(depth, block.close_at);
        self.leave(depth);
    }

    fn stat(&mut self, id: StatId) {
        let ast = self.ast;
        let stat = ast.stat(id);
        let at = stat.span.start;

        match &stat.kind {
            StatKind::Local { names, values } => {
                let dests: Vec<Reg> = names.iter().map(|_| self.reg()).collect();
                self.explist(values, &dests, at);

                for (name, reg) in names.iter().zip(dests) {
                    self.declare(
                        Some(name.name),
                        Some(name.id),
                        reg,
                        name.attribute == Some(Attribute::Close),
                    );
                }
            }
            StatKind::Global { names, values } => {
                if values.is_empty() {
                    return;
                }

                let dests: Vec<Reg> = names.iter().map(|_| self.reg()).collect();
                self.explist(values, &dests, at);

                let access = self
                    .bindings
                    .env(id)
                    .expect("a global assignment reaches _ENV");
                let env = self.access(access, at);

                for (name, src) in names.iter().zip(dests) {
                    let key = self.reg();
                    self.emit(
                        Op::Const {
                            dest: key,
                            value: Const::Str(name.name.into()),
                        },
                        at,
                    );
                    self.emit(Op::DefineGlobal { env, key, src }, at);
                }
            }
            StatKind::GlobalAll { .. } => {}
            StatKind::Assign { targets, values } => {
                let places: Vec<Target> = targets.iter().map(|&at| self.target(at)).collect();
                let dests: Vec<Reg> = places.iter().map(|_| self.reg()).collect();
                self.explist(values, &dests, at);

                for (place, src) in places.into_iter().zip(dests) {
                    self.assign(place, src, at);
                }
            }
            StatKind::Do(body) => self.body(*body),
            StatKind::Expr(value) => self.multi(*value, Some(Box::new([]))),
            StatKind::If { arms, otherwise } => {
                let join = self.new_block();

                for (condition, arm) in arms.iter() {
                    let cond = self.operand(*condition);
                    let then = self.new_block();
                    let next = self.new_block();

                    self.emit(
                        Op::Branch {
                            cond,
                            then,
                            otherwise: next,
                        },
                        at,
                    );

                    self.switch_to(then);
                    self.body(*arm);
                    self.jump_to(join, at);
                    self.switch_to(next);
                }

                if let Some(otherwise) = otherwise {
                    self.body(*otherwise);
                }

                self.jump_to(join, at);
                self.switch_to(join);
            }
            StatKind::While { condition, body } => {
                let head = self.new_block();
                self.jump_to(head, at);
                self.switch_to(head);

                let cond = self.operand(*condition);
                let inside = self.new_block();
                let exit = self.new_block();
                self.emit(
                    Op::Branch {
                        cond,
                        then: inside,
                        otherwise: exit,
                    },
                    at,
                );

                self.switch_to(inside);

                let depth = self.state().vars.len();
                self.state().loops.push((exit, depth));
                self.body(*body);
                self.jump_to(head, at);

                self.state().loops.pop();
                self.switch_to(exit);
            }
            StatKind::Repeat { body, condition } => {
                let head = self.new_block();
                self.jump_to(head, at);
                self.switch_to(head);

                let exit = self.new_block();
                let depth = self.state().vars.len();
                self.state().loops.push((exit, depth));

                // The condition is read inside the body's scope.
                self.stats(ast.block(*body));

                if !self.is_terminated() {
                    let cond = self.operand(*condition);
                    self.close_upvals(depth, at);
                    self.emit(
                        Op::Branch {
                            cond,
                            then: exit,
                            otherwise: head,
                        },
                        at,
                    );
                }

                self.state().loops.pop();
                self.leave(depth);
                self.switch_to(exit);
            }
            StatKind::NumericFor {
                name,
                start,
                limit,
                step,
                body,
            } => {
                let control = self.reg();
                let end = self.reg();
                let stride = self.reg();
                let var = self.reg();

                self.expr(*start, control);
                self.expr(*limit, end);

                match step {
                    Some(step) => self.expr(*step, stride),
                    None => self.emit(
                        Op::Const {
                            dest: stride,
                            value: Const::Int(1),
                        },
                        at,
                    ),
                }

                // ForPrep wants the four in a row, so they are slots rather than temporaries.
                let outer = self.state().vars.len();
                self.declare(None, None, control, false);
                self.declare(None, None, end, false);
                self.declare(None, None, stride, false);

                let inside = self.new_block();
                let exit = self.new_block();
                self.emit(
                    Op::ForPrep {
                        control,
                        limit: end,
                        step: stride,
                        var,
                        body: inside,
                        exit,
                    },
                    at,
                );

                self.switch_to(inside);
                let depth = self.state().vars.len();
                self.declare(Some(name.name), Some(name.id), var, false);
                self.state().loops.push((exit, depth));

                self.stats(ast.block(*body));
                self.close_upvals(depth, at);

                if !self.is_terminated() {
                    self.emit(
                        Op::ForLoop {
                            control,
                            limit: end,
                            step: stride,
                            var,
                            body: inside,
                            exit,
                        },
                        at,
                    );
                }

                self.state().loops.pop();
                self.leave(depth);

                self.switch_to(exit);
                self.leave(outer);
            }
            StatKind::GenericFor { names, exprs, body } => {
                let iterator = self.reg();
                let state = self.reg();
                let control = self.reg();
                let closing = self.reg();
                self.explist(exprs, &[iterator, state, control, closing], at);

                let outer = self.state().vars.len();
                self.declare(None, None, closing, true);

                let head = self.new_block();
                self.jump_to(head, at);
                self.switch_to(head);

                let results: Vec<Reg> = names.iter().map(|_| self.reg()).collect();
                self.emit(
                    Op::Call {
                        callee: iterator,
                        args: Box::new([state, control]),
                        spread: false,
                        results: Results::Exactly(results.clone().into_boxed_slice()),
                    },
                    at,
                );

                // The loop ends on nil, so `false` keeps it going and a truthiness test will not do.
                self.emit(
                    Op::Move {
                        dest: control,
                        src: results[0],
                    },
                    at,
                );

                let nil = self.reg();
                self.emit(
                    Op::Const {
                        dest: nil,
                        value: Const::Nil,
                    },
                    at,
                );

                let alive = self.reg();
                self.emit(
                    Op::Binary {
                        dest: alive,
                        op: BinOp::Ne,
                        left: control,
                        right: nil,
                    },
                    at,
                );

                let inside = self.new_block();
                let exit = self.new_block();
                self.emit(
                    Op::Branch {
                        cond: alive,
                        then: inside,
                        otherwise: exit,
                    },
                    at,
                );

                self.switch_to(inside);
                let depth = self.state().vars.len();

                for (name, reg) in names.iter().zip(results) {
                    self.declare(Some(name.name), Some(name.id), reg, false);
                }

                self.state().loops.push((exit, depth));
                self.stats(ast.block(*body));

                self.close_upvals(depth, at);
                self.jump_to(head, at);

                self.state().loops.pop();
                self.leave(depth);

                self.switch_to(exit);
                self.leave(outer);
            }
            StatKind::Break => {
                let (exit, depth) = *self.state().loops.last().expect("break inside a loop");

                self.close_upvals(depth, at);
                self.emit(Op::Jump { to: exit }, at);
            }
            StatKind::Goto(_) => {
                let target = self.bindings.target(id).expect("a goto reaches a label");
                self.goto(target, at);
            }
            StatKind::Label(_) => {
                let block = self.label_block(id);

                self.jump_to(block, at);
                self.switch_to(block);
                self.reach_label(id, block);
            }
            StatKind::Return(values) => {
                if let [value] = values.as_ref()
                    && self.is_call(*value)
                    && !self.has_close()
                {
                    let (callee, args, spread) = self.callable(*value);
                    self.emit(
                        Op::TailCall {
                            callee,
                            args,
                            spread,
                        },
                        at,
                    );

                    return;
                }

                let (values, spread) = self.explist_open(values);
                self.emit(Op::Return { values, spread }, at);
            }
            StatKind::Function { target, func } => {
                let place = self.target(*target);
                let src = self.reg();

                self.closure(*func, src, at);
                self.assign(place, src, at);
            }
            StatKind::GlobalFunction { name, func } => {
                let src = self.reg();
                self.closure(*func, src, at);

                let access = self
                    .bindings
                    .env(id)
                    .expect("a global function reaches _ENV");
                let env = self.access(access, at);
                let key = self.reg();

                self.emit(
                    Op::Const {
                        dest: key,
                        value: Const::Str((*name).into()),
                    },
                    at,
                );
                self.emit(Op::DefineGlobal { env, key, src }, at);
            }
            StatKind::LocalFunction { name, func } => {
                let reg = self.reg();

                // Declared before the body so that the function can call itself.
                self.declare(Some(name.name), Some(name.id), reg, false);
                self.closure(*func, reg, at);
            }
        }
    }

    /// Where an assignment will put its value. Worked out before the values are evaluated,
    /// because Lua evaluates every expression in the statement before it stores anything.
    fn target(&mut self, id: ExprId) -> Target {
        let ast = self.ast;
        let expr = ast.expr(id);
        let at = expr.span.start;

        match &expr.kind {
            ExprKind::Name(name) => match self.bindings.at(id).expect("a name is resolved") {
                Binding::Variable(Access::Local(var)) => Target::Local(self.lookup(var)),
                Binding::Variable(Access::Upvalue(index)) => Target::Upvalue(index),
                Binding::Global(access) => {
                    let object = self.access(access, at);
                    let key = self.reg();

                    self.emit(
                        Op::Const {
                            dest: key,
                            value: Const::Str((*name).into()),
                        },
                        at,
                    );

                    Target::Index { object, key }
                }
            },
            ExprKind::Index { object, key } => Target::Index {
                object: self.operand(*object),
                key: self.operand(*key),
            },
            kind => unreachable!("{kind:?} is not assignable"),
        }
    }

    /// Stores a value where the target said to put it.
    fn assign(&mut self, place: Target, src: Reg, at: u32) {
        match place {
            Target::Local(dest) => self.emit(Op::Move { dest, src }, at),
            Target::Upvalue(index) => self.emit(Op::SetUpval { index, src }, at),
            Target::Index { object, key } => self.emit(Op::SetIndex { object, key, src }, at),
        }
    }
}
