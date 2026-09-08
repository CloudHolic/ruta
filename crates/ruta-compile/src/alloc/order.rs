//! Putting the blocks in the order they will be emitted in.

use std::mem;

use crate::ir::{Block, BlockIdx, Function, Op};

/// Reorders the blocks into reverse postorder and renumbers every reference to them.
/// The emitter walks the same order, so most jumps end up going forward.
pub(super) fn linearize(func: &mut Function) {
    let order = reverse_postorder(func);
    let mut place = vec![0u32; order.len()];

    for (at, block) in order.iter().enumerate() {
        place[block.0 as usize] = at as u32;
    }

    let mut taken: Vec<Option<Block>> = mem::take(&mut func.blocks).into_iter().map(Some).collect();
    let mut blocks = Vec::with_capacity(order.len());

    for block in &order {
        blocks.push(
            taken[block.0 as usize]
                .take()
                .expect("each block moves once"),
        );
    }

    for block in blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            for target in targets(&mut instr.op) {
                *target = BlockIdx(place[target.0 as usize])
            }
        }
    }

    for slot in func.slots.iter_mut() {
        slot.enters.block = BlockIdx(place[slot.enters.block.0 as usize]);
        slot.leaves.block = BlockIdx(place[slot.leaves.block.0 as usize]);
    }

    func.blocks = blocks;
}

fn targets(op: &mut Op) -> Vec<&mut BlockIdx> {
    match op {
        Op::Jump { to } => vec![to],
        Op::Branch {
            then, otherwise, ..
        } => vec![then, otherwise],
        Op::ForPrep { body, exit, .. } | Op::ForLoop { body, exit, .. } => vec![body, exit],
        _ => Vec::new(),
    }
}

fn reverse_postorder(func: &Function) -> Vec<BlockIdx> {
    let count = func.blocks.len();
    let mut seen = vec![false; count];
    let mut order = Vec::with_capacity(count);
    let mut stack = vec![(0usize, 0usize)];

    seen[0] = true;

    while let Some((at, step)) = stack.pop() {
        let successors: Vec<BlockIdx> = func.blocks[at].successors().collect();

        match successors.get(step) {
            Some(next) => {
                stack.push((at, step + 1));

                let next = next.0 as usize;
                if next < count && !seen[next] {
                    seen[next] = true;
                    stack.push((next, 0));
                }
            }
            None => order.push(BlockIdx(at as u32)),
        }
    }

    order.reverse();

    // A block no jump reaches still holds instructions, so it follows in the order the lowering made it.
    for (at, reached) in seen.iter().enumerate() {
        if !reached {
            order.push(BlockIdx(at as u32));
        }
    }

    order
}
