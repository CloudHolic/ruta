//! Handing out the registers a frame holds.

use std::mem;

use crate::ir::{Function, Op, Reg, Results};

use super::live::{self, Span};
use super::window;

/// One thing that wants registers: a single value, or a run a frame layout wants in a row.
#[derive(Debug)]
struct Item {
    from: u32,
    width: u32,
    first: u32,
    last: u32,
    /// Where the run feeds an instruction that writes upward from its start without bound:
    /// a call, whose frame begins there, or `...` spreading into it.
    clobbers: Vec<u32>,
}

/// Rewrites every virtual register as the one the frame holds it in, and answers with the map
/// so that a nested function can be told where its captures ended up.
pub(super) fn assign(func: &mut Function) -> Vec<Reg> {
    let spans = live::spans(func);
    let regs = func.regs as usize;

    // Declarations sit on a stack, so their depth is already a register that cannot collide with
    // another declaration in scope at the time. Everything else stacks on top of them.
    let base = func
        .slots
        .iter()
        .map(|slot| slot.number + 1)
        .max()
        .unwrap_or(0);

    let mut places: Vec<Option<u32>> = vec![None; regs];
    for slot in &func.slots {
        places[slot.reg.0 as usize] = Some(slot.number);
    }

    let mut items = items(func, &spans, &places);
    items.sort_by_key(|item| item.first);

    let mut taken: Vec<Option<u32>> = Vec::new();
    let mut placed: Vec<u32> = Vec::with_capacity(items.len());
    let mut top = base;

    for (index, item) in items.iter().enumerate() {
        let earlier = || items[..index].iter().zip(&placed);

        // Whatever survives one of this run's calls has to sit below where the call writes.
        let floor = item
            .clobbers
            .iter()
            .flat_map(|&point| {
                earlier()
                    .filter(move |(other, _)| across(other, point))
                    .map(|(other, at)| at + other.width)
            })
            .fold(base, u32::max);

        let at = lowest(&mut taken, floor, item);

        debug_assert!(
            earlier().all(|(other, other_at)| {
                !other.clobbers.iter().any(|&point| across(item, point))
                    || at + item.width <= *other_at
            }),
            "a value live across a call was placed where the call writes"
        );

        for offset in 0..item.width {
            taken[(at + offset) as usize] = Some(item.last);
            places[(item.from + offset) as usize] = Some(at + offset);
        }

        placed.push(at);
        top = top.max(at + item.width);
    }

    let places: Vec<Reg> = places
        .into_iter()
        .map(|place| Reg(place.unwrap_or(0)))
        .collect();

    for block in func.blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            for reg in live::registers(&mut instr.op) {
                *reg = places[reg.0 as usize];
            }
        }
    }

    for slot in func.slots.iter_mut() {
        slot.reg = places[slot.reg.0 as usize];
    }

    func.regs = top;

    places
}

fn items(func: &Function, spans: &[Option<Span>], placed: &[Option<u32>]) -> Vec<Item> {
    let mut runs: Vec<(u32, u32, Vec<u32>)> = Vec::new();
    let mut in_run = vec![false; func.regs as usize];
    let mut point = 0u32;

    for block in &func.blocks {
        let mut low = u32::MAX;
        let mut high = 0;
        let mut clobbers = Vec::new();

        for instr in &block.instrs {
            let here = point;
            point += 1;

            let Some(row) = window::row(&instr.op) else {
                continue;
            };

            // A consumer that spells out no values of its own still ends the run.
            if let Some(from) = window::start(&instr.op) {
                low = low.min(from.0);
                high = high.max(from.0 + row.width());
            }

            if writes_upward(&instr.op) {
                clobbers.push(here);
            }

            if window::produces_multi(&instr.op) {
                continue;
            }

            if low < high {
                for reg in low..high {
                    in_run[reg as usize] = true;
                }

                runs.push((low, high - low, mem::take(&mut clobbers)));
            }

            low = u32::MAX;
            high = 0;
            clobbers.clear();
        }
    }

    let mut items: Vec<Item> = runs
        .into_iter()
        .filter_map(|(from, width, clobbers)| {
            let stretch = (from..from + width)
                .filter_map(|reg| spans[reg as usize])
                .fold(None, join)?;
            Some(Item {
                from,
                width,
                first: stretch.first,
                last: stretch.last,
                clobbers,
            })
        })
        .collect();

    for (reg, span) in spans.iter().enumerate() {
        let Some(span) = span else {
            continue;
        };

        if placed[reg].is_some() || in_run[reg] {
            continue;
        }

        items.push(Item {
            from: reg as u32,
            width: 1,
            first: span.first,
            last: span.last,
            clobbers: Vec::new(),
        });
    }

    items
}

fn writes_upward(op: &Op) -> bool {
    matches!(
        op,
        Op::Call { .. }
            | Op::TailCall { .. }
            | Op::Vararg {
                results: Results::Multi(_),
            }
    )
}

/// Holds a value from before `point` to after it, so that `point` cannot touch it.
fn across(item: &Item, point: u32) -> bool {
    item.first < point && item.last > point
}

fn join(stretch: Option<Span>, span: Span) -> Option<Span> {
    Some(match stretch {
        None => span,
        Some(stretch) => Span {
            first: stretch.first.min(span.first),
            last: stretch.last.max(span.last),
        },
    })
}

/// The lowest run of `width` registers free for the whole of the item's stretch.
/// Items arrive in the order they begin, so a register is free when what it last held has died.
fn lowest(taken: &mut Vec<Option<u32>>, base: u32, item: &Item) -> u32 {
    let mut at = base;

    loop {
        while taken.len() < (at + item.width) as usize {
            taken.push(None);
        }

        match (at..at + item.width)
            .find(|reg| taken[*reg as usize].is_some_and(|last| last >= item.first))
        {
            Some(busy) => at = busy + 1,
            None => return at,
        }
    }
}
