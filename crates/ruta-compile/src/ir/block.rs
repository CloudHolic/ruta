//! Basic blocks.

use super::instr::{Instr, Op};

/// Index into [`super::Function::blocks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockIdx(pub u32);

/// A straight run of instructions ending in a terminator.
#[derive(Debug, Default)]
pub struct Block {
    pub instrs: Vec<Instr>,
}

impl Block {
    /// Where control can go from here.
    pub fn successors(&self) -> impl Iterator<Item = BlockIdx> {
        let pair = match self.instrs.last().map(|instr| &instr.op) {
            Some(Op::Jump { to }) => [Some(*to), None],
            Some(Op::Branch {
                then, otherwise, ..
            }) => [Some(*then), Some(*otherwise)],
            Some(Op::ForPrep { body, exit, .. }) | Some(Op::ForLoop { body, exit, .. }) => {
                [Some(*body), Some(*exit)]
            }
            _ => [None, None],
        };

        pair.into_iter().flatten()
    }
}
