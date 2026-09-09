//! The constants one prototype carries.

use ruta_bytecode::Constant;

use crate::ir::Const;

/// A prototype's constants, in the order they were first named.
#[derive(Debug, Default)]
pub(super) struct Pool {
    constants: Vec<Constant>,
}

impl Pool {
    /// Where this value sits, adding it when it is new.
    /// Nil and the booleans never reach here: each has an instruction of its own.
    pub(super) fn intern(&mut self, value: &Const) -> u32 {
        let wanted = match value {
            Const::Int(value) => Constant::Int(*value),
            Const::Float(value) => Constant::Float(*value),
            Const::Str(value) => Constant::Str(value.clone()),
            Const::Nil | Const::Bool(_) => unreachable!("{value:?} loads without the pool"),
        };

        match self.constants.iter().position(|held| same(held, &wanted)) {
            Some(at) => at as u32,
            None => {
                self.constants.push(wanted);
                self.constants.len() as u32 - 1
            }
        }
    }

    pub(super) fn finish(self) -> Box<[Constant]> {
        self.constants.into_boxed_slice()
    }
}

/// Floats compare by their bits, so that `0.0` and `-0.0` stay apart and a NaN matches itself.
fn same(held: &Constant, wanted: &Constant) -> bool {
    match (held, wanted) {
        (Constant::Int(held), Constant::Int(wanted)) => held == wanted,
        (Constant::Float(held), Constant::Float(wanted)) => held.to_bits() == wanted.to_bits(),
        (Constant::Str(held), Constant::Str(wanted)) => held == wanted,
        _ => false,
    }
}
