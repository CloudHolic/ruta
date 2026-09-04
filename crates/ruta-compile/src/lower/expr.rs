//! Expressions, each one leaving its value in a register it is handed.

use std::mem;

use ruta_syntax::ast::{BinOp as SyntaxBinOp, ExprId, ExprKind, Field, UnOp as SyntaxUnOp, VarId};
use ruta_syntax::scope::{Access, Binding};

use crate::ir::{BinOp, Const, Op, Reg, Results, UnOp};

use super::func::Lowerer;

/// A constructor can hold hundreds of thousands of items and a frame holds 255 registers,
/// so the positional fields are stored in batches rather than all at once.
const FIELDS_PER_STORE: u32 = 50;

impl Lowerer<'_> {
    pub(super) fn expr(&mut self, id: ExprId, dest: Reg) {
        let ast = self.ast;
        let expr = ast.expr(id);
        let at = expr.span.start;

        match &expr.kind {
            ExprKind::Nil => self.emit(constant(dest, Const::Nil), at),
            ExprKind::True => self.emit(constant(dest, Const::Bool(true)), at),
            ExprKind::False => self.emit(constant(dest, Const::Bool(false)), at),
            ExprKind::Int(value) => self.emit(constant(dest, Const::Int(*value)), at),
            ExprKind::Float(value) => self.emit(constant(dest, Const::Float(*value)), at),
            ExprKind::Str(value) => self.emit(constant(dest, Const::Str(value.clone())), at),
            ExprKind::Name(name) => self.name(id, name, dest, at),
            ExprKind::Paren(inner) => self.expr(*inner, dest),
            ExprKind::Index { object, key } => {
                let object = self.operand(*object);
                let key = self.operand(*key);

                self.emit(Op::Index { dest, object, key }, at);
            }
            ExprKind::Unary { op, operand } => {
                let operand = self.operand(*operand);

                self.emit(
                    Op::Unary {
                        dest,
                        op: unary(*op),
                        operand,
                    },
                    at,
                );
            }
            ExprKind::Binary {
                op: SyntaxBinOp::And,
                left,
                right,
            } => self.short_circuit(*left, *right, dest, true, at),
            ExprKind::Binary {
                op: SyntaxBinOp::Or,
                left,
                right,
            } => self.short_circuit(*left, *right, dest, false, at),
            ExprKind::Binary { op, left, right } => {
                let left = self.operand(*left);
                let right = self.operand(*right);

                self.emit(
                    Op::Binary {
                        dest,
                        op: binary(*op),
                        left,
                        right,
                    },
                    at,
                );
            }
            ExprKind::Table(fields) => self.table(fields, dest, at),
            ExprKind::Call { .. } | ExprKind::Method { .. } | ExprKind::Vararg => {
                self.multi(id, Results::Exactly(Box::new([dest])))
            }
            ExprKind::Function(_) => unimplemented!("closures"),
        }
    }

    /// A subexpression, in a register of its own.
    /// Reading a variable copies it, becuase a call evaluated later in the same expression
    /// can assign to it.
    pub(super) fn operand(&mut self, id: ExprId) -> Reg {
        let dest = self.reg();
        self.expr(id, dest);

        dest
    }

    /// Lowers `values` into `dests`, padding with nil and evaluating any surplus.
    pub(super) fn explist(&mut self, values: &[ExprId], dests: &[Reg], at: u32) {
        let Some((&last, rest)) = values.split_last() else {
            for dest in dests.iter().copied() {
                self.emit(constant(dest, Const::Nil), at);
            }

            return;
        };

        for (index, &value) in rest.iter().enumerate() {
            match dests.get(index) {
                Some(&dest) => self.expr(value, dest),
                None => {
                    self.operand(value);
                }
            }
        }

        let tail = dests.get(rest.len()..).unwrap_or(&[]);

        // The instruction fills the remaining destinations itself, nil where a value is missing.
        if self.is_multi(last) {
            self.multi(last, Results::Exactly(tail.into()));

            return;
        }

        match tail.split_first() {
            Some((&dest, more)) => {
                self.expr(last, dest);

                for dest in more.iter().copied() {
                    self.emit(constant(dest, Const::Nil), at);
                }
            }
            None => {
                self.operand(last);
            }
        }
    }

    /// Lowers a list whose values run to however many there are.
    /// The registers hold the leading values, and `ture` means one more instruction left
    /// its results pending.
    pub(super) fn explist_open(&mut self, values: &[ExprId]) -> (Box<[Reg]>, bool) {
        let Some((&last, rest)) = values.split_last() else {
            return (Box::new([]), false);
        };

        let mut regs: Vec<Reg> = rest.iter().map(|&value| self.operand(value)).collect();

        if self.is_multi(last) {
            self.multi(last, Results::Multi);

            return (regs.into_boxed_slice(), true);
        }

        regs.push(self.operand(last));

        (regs.into_boxed_slice(), false)
    }

    /// Whether this expression produces however many values it wants to.
    pub(super) fn is_multi(&self, id: ExprId) -> bool {
        matches!(
            self.ast.expr(id).kind,
            ExprKind::Call { .. } | ExprKind::Method { .. } | ExprKind::Vararg
        )
    }

    /// Lowers a call, a method call or `...`, which are the only expressions whose result count is not fixed.
    /// Nothing may come between one of these and the instruction that consumes what it left pending.
    pub(super) fn multi(&mut self, id: ExprId, results: Results) {
        let ast = self.ast;
        let expr = ast.expr(id);
        let at = expr.span.start;

        match &expr.kind {
            ExprKind::Call { callee, args } => {
                let callee = self.operand(*callee);
                let (args, spread) = self.explist_open(args);

                self.emit(
                    Op::Call {
                        callee,
                        args,
                        spread,
                        results,
                    },
                    at,
                );
            }
            ExprKind::Method { object, name, args } => {
                let object = self.operand(*object);
                let key = self.reg();
                self.emit(constant(key, Const::Str((*name).into())), at);

                let callee = self.reg();
                self.emit(
                    Op::Index {
                        dest: callee,
                        object,
                        key,
                    },
                    at,
                );

                let (rest, spread) = self.explist_open(args);
                let mut args = Vec::with_capacity(rest.len() + 1);
                args.push(object);
                args.extend_from_slice(&rest);

                self.emit(
                    Op::Call {
                        callee,
                        args: args.into_boxed_slice(),
                        spread,
                        results,
                    },
                    at,
                );
            }
            ExprKind::Vararg => self.emit(Op::Vararg { results }, at),
            kind => unreachable!("{kind:?} produces one value"),
        }
    }

    fn table(&mut self, fields: &[Field<'_>], dest: Reg, at: u32) {
        let array_hint = fields
            .iter()
            .filter(|field| matches!(field, Field::Positional(_)))
            .count() as u32;

        self.emit(
            Op::NewTable {
                dest,
                array_hint,
                hash_hint: fields.len() as u32 - array_hint,
            },
            at,
        );

        let mut pending: Vec<Reg> = Vec::new();
        let mut first = 1;

        for (position, field) in fields.iter().enumerate() {
            match field {
                Field::Positional(value) => {
                    if position + 1 == fields.len() && self.is_multi(*value) {
                        self.multi(*value, Results::Multi);
                        self.store(dest, first, mem::take(&mut pending), true, at);

                        return;
                    }

                    pending.push(self.operand(*value));

                    if pending.len() as u32 == FIELDS_PER_STORE {
                        self.store(dest, first, mem::take(&mut pending), false, at);
                        first += FIELDS_PER_STORE;
                    }
                }
                Field::Named { name, value } => {
                    let key = self.reg();
                    self.emit(constant(key, Const::Str((*name).into())), at);

                    let src = self.operand(*value);
                    self.emit(
                        Op::SetIndex {
                            object: dest,
                            key,
                            src,
                        },
                        at,
                    );
                }
                Field::Keyed { key, value } => {
                    let key = self.operand(*key);
                    let src = self.operand(*value);

                    self.emit(
                        Op::SetIndex {
                            object: dest,
                            key,
                            src,
                        },
                        at,
                    );
                }
            }
        }

        if !pending.is_empty() {
            self.store(dest, first, pending, false, at);
        }
    }

    fn store(&mut self, table: Reg, first: u32, values: Vec<Reg>, spread: bool, at: u32) {
        self.emit(
            Op::SetList {
                table,
                first,
                values: values.into_boxed_slice(),
                spread,
            },
            at,
        );
    }

    fn name(&mut self, id: ExprId, name: &[u8], dest: Reg, at: u32) {
        match self.bindings.at(id).expect("a name is resolved") {
            Binding::Variable(Access::Local(var)) => {
                let src = self.lookup(var);
                self.emit(Op::Move { dest, src }, at);
            }
            Binding::Variable(Access::Upvalue(index)) => {
                self.emit(Op::GetUpval { dest, index }, at)
            }
            Binding::Global(access) => {
                let object = self.access(access, at);
                let key = self.reg();

                self.emit(constant(key, Const::Str(name.into())), at);
                self.emit(Op::Index { dest, object, key }, at);
            }
        }
    }

    fn access(&mut self, access: Access, at: u32) -> Reg {
        let dest = self.reg();

        match access {
            Access::Local(var) => {
                let src = self.lookup(var);
                self.emit(Op::Move { dest, src }, at);
            }
            Access::Upvalue(index) => self.emit(Op::GetUpval { dest, index }, at),
        }

        dest
    }

    fn lookup(&mut self, var: VarId) -> Reg {
        self.state()
            .vars
            .iter()
            .rev()
            .find(|(id, _)| *id == var)
            .map(|(_, reg)| *reg)
            .expect("a local is declared before it is read")
    }

    /// `and` and `or`. Both arms write `dest`, which is why the IR is not in SSA form.
    fn short_circuit(&mut self, left: ExprId, right: ExprId, dest: Reg, on_true: bool, at: u32) {
        self.expr(left, dest);

        let rhs = self.new_block();
        let join = self.new_block();
        let (then, otherwise) = if on_true { (rhs, join) } else { (join, rhs) };

        self.emit(
            Op::Branch {
                cond: dest,
                then,
                otherwise,
            },
            at,
        );

        self.switch_to(rhs);
        self.expr(right, dest);
        self.emit(Op::Jump { to: join }, at);
        self.switch_to(join);
    }
}

fn constant(dest: Reg, value: Const) -> Op {
    Op::Const { dest, value }
}

fn unary(op: SyntaxUnOp) -> UnOp {
    match op {
        SyntaxUnOp::Neg => UnOp::Neg,
        SyntaxUnOp::Not => UnOp::Not,
        SyntaxUnOp::Len => UnOp::Len,
        SyntaxUnOp::BNot => UnOp::BNot,
    }
}

fn binary(op: SyntaxBinOp) -> BinOp {
    match op {
        SyntaxBinOp::Add => BinOp::Add,
        SyntaxBinOp::Sub => BinOp::Sub,
        SyntaxBinOp::Mul => BinOp::Mul,
        SyntaxBinOp::Div => BinOp::Div,
        SyntaxBinOp::IDiv => BinOp::IDiv,
        SyntaxBinOp::Mod => BinOp::Mod,
        SyntaxBinOp::Pow => BinOp::Pow,
        SyntaxBinOp::Concat => BinOp::Concat,
        SyntaxBinOp::Eq => BinOp::Eq,
        SyntaxBinOp::Ne => BinOp::Ne,
        SyntaxBinOp::Lt => BinOp::Lt,
        SyntaxBinOp::Le => BinOp::Le,
        SyntaxBinOp::Gt => BinOp::Gt,
        SyntaxBinOp::Ge => BinOp::Ge,
        SyntaxBinOp::BAnd => BinOp::BAnd,
        SyntaxBinOp::BOr => BinOp::BOr,
        SyntaxBinOp::BXor => BinOp::BXor,
        SyntaxBinOp::Shl => BinOp::Shl,
        SyntaxBinOp::Shr => BinOp::Shr,
        SyntaxBinOp::And | SyntaxBinOp::Or => unreachable!("short circuits are not values"),
    }
}
