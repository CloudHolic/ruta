//! What each instruction becomes.

use ruta_bytecode::{MULTI, Op as ByteOp};

use crate::ir::{BinOp, Const, FuncIdx, Op, Reg, Results, UnOp};

use super::pool::Pool;

/// What an instruction needs beyond itself: the pool it names constants in,
/// and the children this function makes, since a closure names one by its place among them.
#[derive(Debug)]
pub(super) struct Context<'a> {
    pub(super) pool: &'a mut Pool,
    pub(super) children: &'a [FuncIdx],
}

/// Every instruction that does not end a block becomes exactly one.
pub(super) fn plain(op: &Op, before: Option<&Op>, cx: &mut Context<'_>) -> ByteOp {
    match op {
        Op::Const { dest, value } => match value {
            Const::Nil => ByteOp::LoadNil { dest: reg(*dest) },
            Const::Bool(true) => ByteOp::LoadTrue { dest: reg(*dest) },
            Const::Bool(false) => ByteOp::LoadFalse { dest: reg(*dest) },
            value => ByteOp::LoadConst {
                dest: reg(*dest),
                constant: cx.pool.intern(value),
            },
        },
        Op::Move { dest, src } => ByteOp::Move {
            dest: reg(*dest),
            src: reg(*src),
        },
        Op::GetUpval { dest, index } => ByteOp::GetUpval {
            dest: reg(*dest),
            index: *index as u8,
        },
        Op::SetUpval { index, src } => ByteOp::SetUpval {
            index: *index as u8,
            src: reg(*src),
        },
        Op::CloseUpvals { from } => ByteOp::CloseUpvals { from: reg(*from) },
        Op::Closure { dest, func } => ByteOp::Closure {
            dest: reg(*dest),
            child: cx
                .children
                .iter()
                .position(|held| held == func)
                .expect("a closure of a child of this function") as u32,
        },
        Op::Vararg { results } => ByteOp::Vararg {
            first: start(results),
            count: count(results),
        },
        Op::NewTable {
            dest,
            array_hint,
            hash_hint,
        } => ByteOp::NewTable {
            dest: reg(*dest),
            array_hint: *array_hint,
            hash_hint: *hash_hint,
        },
        Op::Index { dest, object, key } => ByteOp::Index {
            dest: reg(*dest),
            object: reg(*object),
            key: reg(*key),
        },
        Op::SetIndex { object, key, src } => ByteOp::SetIndex {
            object: reg(*object),
            key: reg(*key),
            src: reg(*src),
        },
        Op::DefineGlobal { env, key, src } => ByteOp::DefineGlobal {
            env: reg(*env),
            key: reg(*key),
            src: reg(*src),
        },
        Op::SetList {
            table,
            first,
            values,
            spread,
        } => {
            let held = row(values, before);

            match spread {
                true => ByteOp::SetListSpread {
                    table: reg(*table),
                    first: held,
                    first_index: *first,
                },
                false => ByteOp::SetList {
                    table: reg(*table),
                    first: held,
                    count: values.len() as u8,
                    first_index: *first,
                },
            }
        }
        Op::Unary { dest, op, operand } => {
            let dest = reg(*dest);
            let operand = reg(*operand);

            match op {
                UnOp::Neg => ByteOp::Neg { dest, operand },
                UnOp::Not => ByteOp::Not { dest, operand },
                UnOp::Len => ByteOp::Len { dest, operand },
                UnOp::BNot => ByteOp::BNot { dest, operand },
            }
        }
        Op::Binary {
            dest,
            op,
            left,
            right,
        } => binary(*op, reg(*dest), reg(*left), reg(*right)),
        Op::Call {
            callee,
            args,
            spread,
            results,
        } => ByteOp::Call {
            callee: reg(*callee),
            args: if *spread { MULTI } else { args.len() as u8 },
            results: count(results),
        },
        terminator => unreachable!("{terminator:?} ends a block"),
    }
}

/// A `Return` or a `TailCall`, which end a block without naming another.
pub(super) fn leaving(op: &Op, before: Option<&Op>) -> ByteOp {
    match op {
        Op::Return { values, spread } => ByteOp::Return {
            first: row(values, before),
            count: if *spread { MULTI } else { values.len() as u8 },
        },
        Op::TailCall {
            callee,
            args,
            spread,
        } => ByteOp::TailCall {
            callee: reg(*callee),
            args: if *spread { MULTI } else { args.len() as u8 },
        },
        other => unreachable!("{other:?} does not leave the function"),
    }
}

/// The register a pending row begins at, when the instruction before left one.
pub(super) fn pending(op: Option<&Op>) -> Option<Reg> {
    match op? {
        Op::Call {
            results: Results::Multi(first),
            ..
        }
        | Op::Vararg {
            results: Results::Multi(first),
        } => Some(*first),
        _ => None,
    }
}

/// Where a row of values begins. A consumer that spells out none of its own takes the answer
/// from the producer standing before it.
fn row(values: &[Reg], before: Option<&Op>) -> u8 {
    match values.first() {
        Some(first) => reg(*first),
        None => pending(before).map_or(0, reg),
    }
}

fn start(results: &Results) -> u8 {
    match results {
        Results::Exactly(regs) => regs.first().copied().map_or(0, reg),
        Results::Multi(first) => reg(*first),
    }
}

fn count(results: &Results) -> u8 {
    match results {
        Results::Exactly(regs) => regs.len() as u8,
        Results::Multi(_) => MULTI,
    }
}

fn reg(Reg(index): Reg) -> u8 {
    index as u8
}

fn binary(op: BinOp, dest: u8, left: u8, right: u8) -> ByteOp {
    match op {
        BinOp::Add => ByteOp::Add { dest, left, right },
        BinOp::Sub => ByteOp::Sub { dest, left, right },
        BinOp::Mul => ByteOp::Mul { dest, left, right },
        BinOp::Div => ByteOp::Div { dest, left, right },
        BinOp::IDiv => ByteOp::IDiv { dest, left, right },
        BinOp::Mod => ByteOp::Mod { dest, left, right },
        BinOp::Pow => ByteOp::Pow { dest, left, right },
        BinOp::Concat => ByteOp::Concat { dest, left, right },
        BinOp::Eq => ByteOp::Eq { dest, left, right },
        BinOp::Ne => ByteOp::Ne { dest, left, right },
        BinOp::Lt => ByteOp::Lt { dest, left, right },
        BinOp::Le => ByteOp::Le { dest, left, right },
        BinOp::Gt => ByteOp::Gt { dest, left, right },
        BinOp::Ge => ByteOp::Ge { dest, left, right },
        BinOp::BAnd => ByteOp::BAnd { dest, left, right },
        BinOp::BOr => ByteOp::BOr { dest, left, right },
        BinOp::BXor => ByteOp::BXor { dest, left, right },
        BinOp::Shl => ByteOp::Shl { dest, left, right },
        BinOp::Shr => ByteOp::Shr { dest, left, right },
    }
}
