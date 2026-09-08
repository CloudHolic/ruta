//! Fitting each function into the registers a frame holds.

use crate::ir::{FuncIdx, Function, Op, Program, UpvalSource};

use super::{assign, order, window};

/// Lowers virtual registers to the physical ones a frame holds.
pub fn allocate(program: &mut Program) {
    for index in 0..program.funcs.len() {
        let func = &mut program.funcs[index];

        window::materialize(func);
        order::linearize(func);

        let places = assign::assign(func);

        for child in children(&program.funcs[index]) {
            for source in program.funcs[child.0 as usize].upvalues.iter_mut() {
                if let UpvalSource::ParentLocal(reg) = source {
                    *reg = places[reg.0 as usize];
                }
            }
        }
    }
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
