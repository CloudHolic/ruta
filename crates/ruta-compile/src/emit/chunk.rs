//! Assembling the prototype each funciton becomes.

use ruta_bytecode::{
    Encoder, LineTable, LocalVar, Op as ByteOp, Prototype, UpvalDesc, UpvalSource as ByteUpval,
    Vararg as ByteVararg,
};
use ruta_syntax::line_index::LineIndex;

use crate::ir::{BlockIdx, FuncIdx, Function, Op, Position, Program, UpvalSource, Vararg};

use super::instr::{self, Context};
use super::pool::Pool;

/// WHat the prototype needs that the IR does not carry: the source it  was compiled from.
#[derive(Debug)]
pub struct Source<'a> {
    pub lines: &'a LineIndex,
    /// The chunk name, already in the form a message shows.
    pub name: &'a [u8],
}

/// Turns a compiled chunk into the prototype tree that runs it.
pub fn emit(program: &Program, source: &Source<'_>) -> Prototype {
    prototype(program, 0, source)
}

fn prototype(program: &Program, index: usize, source: &Source<'_>) -> Prototype {
    let func = &program.funcs[index];
    let children = closures(func);
    let mut pool = Pool::default();
    let (code, lines, spots) = write(func, &children, &mut pool, source);

    Prototype {
        params: func.params as u8,
        vararg: match func.vararg {
            Vararg::None => ByteVararg::None,
            Vararg::Anonymous => ByteVararg::Anonymous,
            Vararg::Table => ByteVararg::Table,
        },
        max_registers: func.regs as u8,
        code,
        constants: pool.finish(),
        children: children
            .iter()
            .map(|child| prototype(program, child.0 as usize, source))
            .collect(),
        upvals: func
            .upvalues
            .iter()
            .map(|upvalue| UpvalDesc {
                name: upvalue.name.clone(),
                source: match upvalue.source {
                    UpvalSource::ParentLocal(reg) => ByteUpval::ParentLocal(reg.0 as u8),
                    UpvalSource::ParentUpval(at) => ByteUpval::ParentUpval(at as u8),
                    UpvalSource::Env => ByteUpval::ParentLocal(0),
                },
            })
            .collect(),
        source: source.name.into(),
        line_defined: defined(index, func, source),
        last_line_defined: last_defined(index, func, source),
        lines,
        locals: func
            .slots
            .iter()
            .filter_map(|slot| {
                let name = slot.name.clone()?;
                let start_pc = spot(&spots, slot.enters);
                let end_pc = spot(&spots, slot.leaves);

                // A goto over a declaration to the end of its block leaves the declaration
                // where nothing reaches it, and no pc is then inside the scope.
                (start_pc <= end_pc).then_some(LocalVar {
                    name,
                    register: slot.reg.0 as u8,
                    start_pc,
                    end_pc,
                })
            })
            .collect(),
    }
}

/// The children this funciton makes, in the order it makes them.
/// A closure names one by its place in this list, which is what the prototype tree indexes by.
fn closures(func: &Function) -> Vec<FuncIdx> {
    func.blocks
        .iter()
        .flat_map(|block| block.instrs.iter())
        .filter_map(|instr| match instr.op {
            Op::Closure { func, .. } => Some(func),
            _ => None,
        })
        .collect()
}

/// Lays the blocks out end to end.
/// Jumps go out with a placeholder and are corrected once every block's beginning is known.
fn write(
    func: &Function,
    children: &[FuncIdx],
    pool: &mut Pool,
    source: &Source<'_>,
) -> (Box<[u8]>, LineTable, Vec<Vec<u32>>) {
    let mut encoder = Encoder::new();
    let mut cx = Context { pool, children };
    let mut starts = vec![0u32; func.blocks.len()];
    let mut spots: Vec<Vec<u32>> = Vec::with_capacity(func.blocks.len());
    let mut waiting: Vec<(u32, BlockIdx)> = Vec::new();

    for (at, block) in func.blocks.iter().enumerate() {
        starts[at] = encoder.pc();

        let next = (at + 1 < func.blocks.len()).then(|| BlockIdx(at as u32 + 1));
        let mut here = Vec::with_capacity(block.instrs.len() + 1);

        for (position, instr) in block.instrs.iter().enumerate() {
            let line = source.lines.line_of(instr.at);
            let before = position.checked_sub(1).map(|at| &block.instrs[at].op);
            here.push(encoder.pc());

            if position + 1 != block.instrs.len() {
                encoder.emit(&instr::plain(&instr.op, before, &mut cx), line);
                continue;
            }

            for (op, target) in tail(&instr.op, before, next) {
                let pc = encoder.emit(&op, line);

                if let Some(target) = target {
                    waiting.push((pc, target));
                }
            }
        }

        here.push(encoder.pc());
        spots.push(here);
    }

    for (pc, target) in waiting {
        encoder.patch_jump(pc, starts[target.0 as usize]);
    }

    let (code, lines) = encoder.finish();

    (code, lines, spots)
}

fn tail(op: &Op, before: Option<&Op>, next: Option<BlockIdx>) -> Vec<(ByteOp, Option<BlockIdx>)> {
    match op {
        Op::Jump { to } => match next == Some(*to) {
            true => Vec::new(),
            false => vec![(ByteOp::Jump { offset: 0 }, Some(*to))],
        },
        Op::Branch {
            cond,
            then,
            otherwise,
        } => {
            let cond = cond.0 as u8;

            if next == Some(*otherwise) {
                vec![(ByteOp::JumpIfTrue { cond, offset: 0 }, Some(*then))]
            } else if next == Some(*then) {
                vec![(ByteOp::JumpIfFalse { cond, offset: 0 }, Some(*otherwise))]
            } else {
                vec![
                    (ByteOp::JumpIfTrue { cond, offset: 0 }, Some(*then)),
                    (ByteOp::Jump { offset: 0 }, Some(*otherwise)),
                ]
            }
        }
        Op::ForPrep {
            control,
            body,
            exit,
            ..
        } => {
            let control = control.0 as u8;
            let mut out = vec![(ByteOp::ForPrep { control, offset: 0 }, Some(*exit))];

            if next != Some(*body) {
                out.push((ByteOp::Jump { offset: 0 }, Some(*body)));
            }

            out
        }
        Op::ForLoop {
            control,
            body,
            exit,
            ..
        } => {
            let control = control.0 as u8;
            let mut out = vec![(ByteOp::ForLoop { control, offset: 0 }, Some(*body))];

            if next != Some(*exit) {
                out.push((ByteOp::Jump { offset: 0 }, Some(*exit)));
            }

            out
        }
        leaving => vec![(instr::leaving(leaving, before), None)],
    }
}

fn spot(spots: &[Vec<u32>], at: Position) -> u32 {
    spots[at.block.0 as usize][at.instr as usize]
}

fn defined(index: usize, func: &Function, source: &Source<'_>) -> u32 {
    match index {
        0 => 0,
        _ => source.lines.line_of(func.span.start),
    }
}

fn last_defined(index: usize, func: &Function, source: &Source<'_>) -> u32 {
    match index {
        0 => 0,
        _ => source.lines.line_of(func.span.end - 1),
    }
}
