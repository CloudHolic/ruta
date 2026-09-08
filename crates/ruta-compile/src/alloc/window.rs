//! Making the runs of registers the frame layout demands explicit.
//!
//! A call takes its function and arguments in a row and leaves its results in the same place;
//! a return, a constructor's positional batch and `...` each want a row of their own.
//! The lowering hands them registers it chose freely, so each one becomes a fresh row that `Move`s fill right before it,
//! and the allocator is left wihth one question: where does the row go.

use std::mem;

use crate::ir::{Function, Instr, Op, Reg, Results};

/// The row one instruction wants: what it reads from it, and what it leaves in it,
/// and whether it leaves values running to the top of the frame.
#[derive(Debug)]
pub(super) struct Row {
    inputs: Vec<Reg>,
    outputs: Vec<Reg>,
    open: bool,
}

impl Row {
    pub(super) fn width(&self) -> u32 {
        // Values left pending still start somewhere, even where nothing else claims the row.
        let least = usize::from(self.open);

        self.inputs.len().max(self.outputs.len()).max(least) as u32
    }
}

pub(super) fn materialize(func: &mut Function) {
    let mut blocks = mem::take(&mut func.blocks);
    let mut regs = func.regs;

    for block in blocks.iter_mut() {
        let instrs = mem::take(&mut block.instrs);
        block.instrs = rewrite(instrs, &mut regs);
    }

    func.blocks = blocks;
    func.regs = regs;
}

/// Where the row an instruction wants begins, once it has one.
pub(super) fn start(op: &Op) -> Option<Reg> {
    match op {
        Op::Call { callee, .. } | Op::TailCall { callee, .. } => Some(*callee),
        Op::Return { values, .. } | Op::SetList { values, .. } => values.first().copied(),
        Op::Vararg { results } => match results {
            Results::Exactly(regs) => regs.first().copied(),
            Results::Multi(reg) => Some(*reg),
        },
        _ => None,
    }
}

pub(super) fn row(op: &Op) -> Option<Row> {
    let (inputs, outputs) = match op {
        Op::Call {
            callee,
            args,
            results,
            ..
        } => {
            let mut inputs = vec![*callee];
            inputs.extend_from_slice(args);

            (inputs, destinations(results))
        }
        Op::TailCall { callee, args, .. } => {
            let mut inputs = vec![*callee];
            inputs.extend_from_slice(args);

            (inputs, Vec::new())
        }
        Op::Return { values, .. } | Op::SetList { values, .. } => (values.to_vec(), Vec::new()),
        Op::Vararg { results } => (Vec::new(), destinations(results)),
        _ => return None,
    };

    Some(Row {
        inputs,
        outputs,
        open: produces_multi(op),
    })
}

pub(super) fn produces_multi(op: &Op) -> bool {
    matches!(
        op,
        Op::Call {
            results: Results::Multi(_),
            ..
        } | Op::Vararg {
            results: Results::Multi(_),
        }
    )
}

/// Values left pending run to the top of the frame, so the instruction that spreads them
/// and every producer feeding it settle their rows together.
fn rewrite(instrs: Vec<Instr>, regs: &mut u32) -> Vec<Instr> {
    let mut out = Vec::with_capacity(instrs.len());
    let mut run: Vec<Instr> = Vec::new();

    for instr in instrs {
        let pending = produces_multi(&instr.op);
        run.push(instr);

        if !pending {
            place(mem::take(&mut run), regs, &mut out);
        }
    }

    debug_assert!(run.is_empty());

    out
}

fn place(run: Vec<Instr>, regs: &mut u32, out: &mut Vec<Instr>) {
    let rows: Vec<Option<Row>> = run.iter().map(|instr| row(&instr.op)).collect();

    if rows.len() == 1 && rows[0].is_none() {
        out.extend(run);

        return;
    }

    let lines: Vec<u32> = run.iter().map(|instr| instr.at).collect();
    let mut bases = vec![Reg(0); run.len()];
    let mut cursor = *regs;
    let mut top = *regs;

    // The instruction that consumes comes last and sits lowest;
    // each producer stacks on the arguments already spelled out below it.
    for at in (0..run.len()).rev() {
        let row = rows[at].as_ref().expect("a run holds rows only");

        bases[at] = Reg(cursor);
        cursor += row.inputs.len() as u32;
        top = top.max(bases[at].0 + row.width());
    }

    *regs = top.max(cursor);

    for at in (0..run.len()).rev() {
        let row = rows[at].as_ref().expect("a run holdds rows only");

        for (offset, src) in row.inputs.iter().enumerate() {
            out.push(Instr {
                op: Op::Move {
                    dest: Reg(bases[at].0 + offset as u32),
                    src: *src,
                },
                at: lines[at],
            })
        }
    }

    for (at, instr) in run.into_iter().enumerate() {
        out.push(Instr {
            op: settle(instr.op, bases[at]),
            at: instr.at,
        });
    }

    for at in 0..bases.len() {
        let row = rows[at].as_ref().expect("a run holds rows only");

        for (offset, dest) in row.outputs.iter().enumerate() {
            out.push(Instr {
                op: Op::Move {
                    dest: *dest,
                    src: Reg(bases[at].0 + offset as u32),
                },
                at: lines[at],
            });
        }
    }
}

fn destinations(results: &Results) -> Vec<Reg> {
    match results {
        Results::Exactly(regs) => regs.to_vec(),
        Results::Multi(_) => Vec::new(),
    }
}

fn settle(op: Op, base: Reg) -> Op {
    match op {
        Op::Call {
            args,
            spread,
            results,
            ..
        } => Op::Call {
            callee: base,
            args: consecutive(base.0 + 1, args.len()),
            spread,
            results: land(results, base),
        },
        Op::TailCall { args, spread, .. } => Op::TailCall {
            callee: base,
            args: consecutive(base.0 + 1, args.len()),
            spread,
        },
        Op::Return { values, spread } => Op::Return {
            values: consecutive(base.0, values.len()),
            spread,
        },
        Op::SetList {
            table,
            first,
            values,
            spread,
        } => Op::SetList {
            table,
            first,
            values: consecutive(base.0, values.len()),
            spread,
        },
        Op::Vararg { results } => Op::Vararg {
            results: land(results, base),
        },
        other => other,
    }
}

fn land(results: Results, base: Reg) -> Results {
    match results {
        Results::Exactly(regs) => Results::Exactly(consecutive(base.0, regs.len())),
        Results::Multi(_) => Results::Multi(base),
    }
}

fn consecutive(from: u32, count: usize) -> Box<[Reg]> {
    (0..count as u32).map(|offset| Reg(from + offset)).collect()
}
