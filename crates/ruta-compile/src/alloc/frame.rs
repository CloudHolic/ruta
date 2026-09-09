//! Fitting each function into the registers a frame holds.

use ruta_syntax::error::{Error, ErrorKind, Near};

use crate::ir::{FuncIdx, Function, Op, Program, UpvalSource};

use super::{assign, order, window};

/// A register index is one byte wide, and the widest value marks a count that runs to the top of the frame.
const REGISTERS: u32 = 255;
/// Declarations in scope at one point, which the frame holds below everything else.
const LOCALS: u32 = 200;
/// One less than the message names, the same way returns are counted.
const UPVALUES: usize = 255;

/// Lowers virtual registers to the physical ones a frame holds.
pub fn allocate(program: &mut Program) -> Result<(), Error> {
    for index in 0..program.funcs.len() {
        let func = &mut program.funcs[index];

        window::materialize(func);
        order::linearize(func);

        let places = assign::assign(func);

        limits(index, &program.funcs[index])?;

        // A nested function names a register of the one that makes it, so it learns the answer here.
        // Functions come parent first, so the answer is always ready.
        for child in children(&program.funcs[index]) {
            for source in program.funcs[child.0 as usize].upvalues.iter_mut() {
                if let UpvalSource::ParentLocal(reg) = source {
                    *reg = places[reg.0 as usize];
                }
            }
        }
    }

    Ok(())
}

/// What a frame cannot hold.
fn limits(index: usize, func: &Function) -> Result<(), Error> {
    let declared = func
        .slots
        .iter()
        .map(|slot| slot.number + 1)
        .max()
        .unwrap_or(0);

    let crossed = if declared > LOCALS {
        Some(("local variables", 200))
    } else if func.upvalues.len() > UPVALUES {
        Some(("upvalues", 255))
    } else if func.regs > REGISTERS {
        Some(("registers", 255))
    } else {
        None
    };

    let Some((what, limit)) = crossed else {
        return Ok(());
    };

    Err(Error {
        kind: ErrorKind::TooMany {
            what,
            limit,
            function: (index != 0).then_some(func.span.start),
        },
        at: func.span.end,
        near: if index == 0 {
            Near::Eof
        } else {
            Near::Buffer(b"end".to_vec())
        },
    })
}

fn children(func: &Function) -> Vec<FuncIdx> {
    func.blocks
        .iter()
        .flat_map(|block| block.instrs.iter())
        .filter_map(|instr| match instr.op {
            Op::Closure { func, .. } => Some(func),
            _ => None,
        })
        .collect()
}
