//! Where each virtual register has to survive.

use crate::ir::{Function, Op, Reg, Results};

/// The stretch of the instruction stream a register has to survive,
/// counted over the instructions of every block in block order.
#[derive(Debug, Clone, Copy)]
pub(super) struct Span {
    pub(super) first: u32,
    pub(super) last: u32,
}

#[derive(Debug, Clone)]
struct Bits {
    words: Vec<u64>,
}

impl Bits {
    fn new(len: usize) -> Bits {
        Bits {
            words: vec![0; len.div_ceil(64)],
        }
    }

    fn get(&self, index: u32) -> bool {
        let index = index as usize;

        self.words
            .get(index / 64)
            .is_some_and(|word| word & (1u64 << (index % 64)) != 0)
    }

    fn set(&mut self, index: u32) {
        let index = index as usize;

        if let Some(word) = self.words.get_mut(index / 64) {
            *word |= 1u64 << (index % 64);
        }
    }

    fn union(&mut self, other: &Bits) -> bool {
        let mut changed = false;

        for (mine, theirs) in self.words.iter_mut().zip(&other.words) {
            let next = *mine | *theirs;
            changed |= next != *mine;
            *mine = next;
        }

        changed
    }

    fn each(&self, mut visit: impl FnMut(u32)) {
        for (at, word) in self.words.iter().enumerate() {
            let mut word = *word;

            while word != 0 {
                let bit = word.trailing_zeros();
                visit(at as u32 * 64 + bit);
                word &= word - 1;
            }
        }
    }
}

/// The span of every virtual register, or `None` where no instruction names it.
pub(super) fn spans(func: &Function) -> Vec<Option<Span>> {
    let regs = func.regs as usize;
    let starts = starts(func);
    let (upward, written) = block_sets(func, regs);
    let incoming = solve(func, &upward, &written, regs);

    let mut spans: Vec<Option<Span>> = vec![None; regs];

    for (at, block) in func.blocks.iter().enumerate() {
        let start = starts[at];
        let end = start + block.instrs.len() as u32 - 1;

        incoming[at].each(|reg| extend(&mut spans, reg, start));

        for target in block.successors() {
            incoming[target.0 as usize].each(|reg| extend(&mut spans, reg, end));
        }

        for (offset, instr) in block.instrs.iter().enumerate() {
            let point = start + offset as u32;

            for reg in reads(&instr.op).into_iter().chain(writes(&instr.op)) {
                extend(&mut spans, reg.0, point);
            }
        }
    }

    spans
}

pub(super) fn reads(op: &Op) -> Vec<Reg> {
    match op {
        Op::Move { src, .. } | Op::SetUpval { src, .. } => vec![*src],
        Op::Index { object, key, .. } => vec![*object, *key],
        Op::SetIndex { object, key, src } => vec![*object, *key, *src],
        Op::DefineGlobal { env, key, src } => vec![*env, *key, *src],
        Op::SetList { table, values, .. } => {
            let mut regs = vec![*table];
            regs.extend_from_slice(values);
            regs
        }
        Op::Unary { operand, .. } => vec![*operand],
        Op::Binary { left, right, .. } => vec![*left, *right],
        Op::Call { callee, args, .. } | Op::TailCall { callee, args, .. } => {
            let mut regs = vec![*callee];
            regs.extend_from_slice(args);
            regs
        }
        Op::Branch { cond, .. } => vec![*cond],
        Op::Return { values, .. } => values.to_vec(),
        Op::ForPrep {
            control,
            limit,
            step,
            ..
        }
        | Op::ForLoop {
            control,
            limit,
            step,
            ..
        } => vec![*control, *limit, *step],
        Op::Const { .. }
        | Op::GetUpval { .. }
        | Op::Closure { .. }
        | Op::Vararg { .. }
        | Op::NewTable { .. }
        | Op::CloseUpvals { .. }
        | Op::Jump { .. } => Vec::new(),
    }
}

pub(super) fn writes(op: &Op) -> Vec<Reg> {
    match op {
        Op::Const { dest, .. }
        | Op::Move { dest, .. }
        | Op::GetUpval { dest, .. }
        | Op::Closure { dest, .. }
        | Op::NewTable { dest, .. }
        | Op::Index { dest, .. }
        | Op::Unary { dest, .. }
        | Op::Binary { dest, .. } => vec![*dest],
        Op::Call { results, .. } | Op::Vararg { results } => match results {
            Results::Exactly(regs) => regs.to_vec(),
            Results::Multi(reg) => vec![*reg],
        },
        Op::ForPrep { control, var, .. } | Op::ForLoop { control, var, .. } => {
            vec![*control, *var]
        }
        _ => Vec::new(),
    }
}

pub(super) fn registers(op: &mut Op) -> Vec<&mut Reg> {
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

fn extend(spans: &mut [Option<Span>], reg: u32, point: u32) {
    match &mut spans[reg as usize] {
        Some(span) => {
            span.first = span.first.min(point);
            span.last = span.last.max(point);
        }
        slot => {
            *slot = Some(Span {
                first: point,
                last: point,
            })
        }
    }
}

fn starts(func: &Function) -> Vec<u32> {
    let mut starts = Vec::with_capacity(func.blocks.len());
    let mut next = 0;

    for block in &func.blocks {
        starts.push(next);
        next += block.instrs.len() as u32;
    }

    starts
}

/// Per block: the registers read before the block writes them, and the ones it writes.
fn block_sets(func: &Function, regs: usize) -> (Vec<Bits>, Vec<Bits>) {
    let mut upward = Vec::with_capacity(func.blocks.len());
    let mut written = Vec::with_capacity(func.blocks.len());

    for block in &func.blocks {
        let mut reads_first = Bits::new(regs);
        let mut writes_first = Bits::new(regs);

        for instr in &block.instrs {
            for reg in reads(&instr.op) {
                if !writes_first.get(reg.0) {
                    reads_first.set(reg.0);
                }
            }

            for reg in writes(&instr.op) {
                writes_first.set(reg.0);
            }
        }

        upward.push(reads_first);
        written.push(writes_first);
    }

    (upward, written)
}

fn solve(func: &Function, upward: &[Bits], written: &[Bits], regs: usize) -> Vec<Bits> {
    let mut incoming: Vec<Bits> = upward.to_vec();

    loop {
        let mut changed = false;

        for at in (0..func.blocks.len()).rev() {
            let mut outgoing = Bits::new(regs);

            for target in func.blocks[at].successors() {
                outgoing.union(&incoming[target.0 as usize]);
            }

            let mut through = outgoing;
            for (word, killed) in through.words.iter_mut().zip(&written[at].words) {
                *word &= !*killed;
            }

            changed |= incoming[at].union(&through);
        }

        if !changed {
            break;
        }
    }

    incoming
}

fn landed(results: &mut Results) -> Vec<&mut Reg> {
    match results {
        Results::Exactly(regs) => regs.iter_mut().collect(),
        Results::Multi(reg) => vec![reg],
    }
}
