//! Fitting a function into the registers a frame holds.

use crate::ir::Program;

use super::window;

/// Lowers virtual registers to the physical ones a frame holds.
pub fn allocate(program: &mut Program) {
    for func in program.funcs.iter_mut() {
        window::materialize(func);
    }
}
