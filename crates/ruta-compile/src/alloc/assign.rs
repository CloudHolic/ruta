//! Handing out the registers a frame holds.

use crate::ir::{Function, Op, Reg, Results};

use super::live::{self, Span};
use super::window;

/// One thing that wants registers: a single vlaue, or a run a frame layout wants in a row.
#[derive(Debug)]
struct Item {
    from: u32,
    width: u32,
    first: u32,
    last: u32,
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
    let mut top = base;

    for item in &items {
        let at = lowest(&mut taken, base, item);

        for offset in 0..item.width {
            taken[(at + offset) as usize] = Some(item.last);
            places[(item.from + offset) as usize] = Some(at + offset);
        }

        top = top.max(at + item.width);
    }

    let places: Vec<Reg> = places
        .into_iter()
        .map(|place| Reg(place.unwrap_or(0)))
        .collect();

    for block in func.blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            for reg in registers(&mut instr.op) {
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
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut in_run = vec![false; func.regs as usize];

    for block in &func.blocks {
        let mut low = u32::MAX;
        let mut high = 0;

        for instr in &block.instrs {
            let Some(row) = window::row(&instr.op) else {
                continue;
            };

            // A consumer that spells out no values of its own still ends the run.
            if let Some(from) = window::start(&instr.op) {
                low = low.min(from.0);
                high = high.max(from.0 + row.width());
            }

            if window::produces_multi(&instr.op) {
                continue;
            }

            if low < high {
                for reg in low..high {
                    in_run[reg as usize] = true;
                }

                runs.push((low, high - low));
            }

            low = u32::MAX;
            high = 0;
        }
    }

    let mut items: Vec<Item> = runs
        .into_iter()
        .filter_map(|(from, width)| {
            let stretch = (from..from + width)
                .filter_map(|reg| spans[reg as usize])
                .fold(None, join)?;
            Some(Item {
                from,
                width,
                first: stretch.first,
                last: stretch.last,
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
        });
    }

    items
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

fn registers(op: &mut Op) -> Vec<&mut Reg> {
    match op {
        Op::Const { dest, .. }
        | Op::GetUpval { dest, .. }
        | Op::Closure { dest, .. }
        | Op::NewTable { dest, .. } => vec![dest],
        Op::Move { dest, src } => vec![dest, src],
        Op::SetUpval { src, .. } => vec![src],
        Op::CloseUpvals { from } => vec![from],
        Op::Vararg { results } => landed(results),
        Op::Index { dest, object, key } => vec![dest, object, key],
        Op::SetIndex { object, key, src }
        | Op::DefineGlobal {
            env: object,
            key,
            src,
        } => {
            vec![object, key, src]
        }
        Op::SetList { table, values, .. } => {
            let mut regs = vec![table];
            regs.extend(values.iter_mut());
            regs
        }
        Op::Unary { dest, operand, .. } => vec![dest, operand],
        Op::Binary {
            dest, left, right, ..
        } => vec![dest, left, right],
        Op::Call {
            callee,
            args,
            results,
            ..
        } => {
            let mut regs = vec![callee];
            regs.extend(args.iter_mut());
            regs.extend(landed(results));
            regs
        }
        Op::TailCall { callee, args, .. } => {
            let mut regs = vec![callee];
            regs.extend(args.iter_mut());
            regs
        }
        Op::Branch { cond, .. } => vec![cond],
        Op::Return { values, .. } => values.iter_mut().collect(),
        Op::ForPrep {
            control,
            limit,
            step,
            var,
            ..
        }
        | Op::ForLoop {
            control,
            limit,
            step,
            var,
            ..
        } => vec![control, limit, step, var],
        Op::Jump { .. } => Vec::new(),
    }
}

fn landed(results: &mut Results) -> Vec<&mut Reg> {
    match results {
        Results::Exactly(regs) => regs.iter_mut().collect(),
        Results::Multi(reg) => vec![reg],
    }
}
