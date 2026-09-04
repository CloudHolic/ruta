//! Turning instructions into bytes.

use crate::debug::LineTable;
use crate::op::{Op, OpCode, instruction_len};

/// Appends instructions to a code buffer, building the line table as it goes.
#[derive(Debug, Default)]
pub struct Encoder {
    code: Vec<u8>,
    lines: Vec<(u32, u32)>,
}

impl Encoder {
    pub fn new() -> Encoder {
        Encoder::default()
    }

    /// Where the next instruction will begin.
    pub fn pc(&self) -> u32 {
        self.code.len() as u32
    }

    pub fn finish(self) -> (Box<[u8]>, LineTable) {
        (
            self.code.into_boxed_slice(),
            LineTable::new(self.lines.into_boxed_slice()),
        )
    }

    /// Appends one instruction and returns the pc it begins at.
    pub fn emit(&mut self, op: &Op, line: u32) -> u32 {
        let at = self.pc();

        if self.lines.last().map(|entry| entry.1) != Some(line) {
            self.lines.push((at, line));
        }

        match *op {
            Op::LoadNil { dest } => self.write(OpCode::LoadNil, &[dest.into()]),
            Op::LoadTrue { dest } => self.write(OpCode::LoadTrue, &[dest.into()]),
            Op::LoadFalse { dest } => self.write(OpCode::LoadFalse, &[dest.into()]),
            Op::LoadConst { dest, constant } => match u16::try_from(constant) {
                Ok(narrow) => self.write(OpCode::LoadConst, &[dest.into(), narrow.into()]),
                Err(_) => self.write(OpCode::LoadConstWide, &[dest.into(), constant]),
            },
            Op::Move { dest, src } => self.write(OpCode::Move, &[dest.into(), src.into()]),

            Op::GetUpval { dest, index } => {
                self.write(OpCode::GetUpval, &[dest.into(), index.into()])
            }
            Op::SetUpval { index, src } => {
                self.write(OpCode::SetUpval, &[index.into(), src.into()])
            }
            Op::CloseUpvals { from } => self.write(OpCode::CloseUpvals, &[from.into()]),
            Op::Closure { dest, child } => self.write(OpCode::Closure, &[dest.into(), child]),
            Op::Vararg { first, count } => {
                self.write(OpCode::Vararg, &[first.into(), count.into()])
            }
            Op::NewTable {
                dest,
                array_hint,
                hash_hint,
            } => self.write(OpCode::NewTable, &[dest.into(), array_hint, hash_hint]),
            Op::Index { dest, object, key } => {
                self.write(OpCode::Index, &[dest.into(), object.into(), key.into()])
            }
            Op::SetIndex { object, key, src } => {
                self.write(OpCode::SetIndex, &[object.into(), key.into(), src.into()])
            }
            Op::DefineGlobal { env, key, src } => {
                self.write(OpCode::DefineGlobal, &[env.into(), key.into(), src.into()])
            }
            Op::SetList {
                table,
                first,
                count,
                first_index,
            } => self.write(
                OpCode::SetList,
                &[table.into(), first.into(), count.into(), first_index],
            ),
            Op::SetListSpread {
                table,
                first,
                first_index,
            } => self.write(
                OpCode::SetListSpread,
                &[table.into(), first.into(), first_index],
            ),
            Op::Neg { dest, operand } => self.write(OpCode::Neg, &[dest.into(), operand.into()]),
            Op::Not { dest, operand } => self.write(OpCode::Not, &[dest.into(), operand.into()]),
            Op::Len { dest, operand } => self.write(OpCode::Len, &[dest.into(), operand.into()]),
            Op::BNot { dest, operand } => self.write(OpCode::BNot, &[dest.into(), operand.into()]),

            Op::Add { dest, left, right } => {
                self.write(OpCode::Add, &[dest.into(), left.into(), right.into()])
            }
            Op::Sub { dest, left, right } => {
                self.write(OpCode::Sub, &[dest.into(), left.into(), right.into()])
            }
            Op::Mul { dest, left, right } => {
                self.write(OpCode::Mul, &[dest.into(), left.into(), right.into()])
            }
            Op::Div { dest, left, right } => {
                self.write(OpCode::Div, &[dest.into(), left.into(), right.into()])
            }
            Op::IDiv { dest, left, right } => {
                self.write(OpCode::IDiv, &[dest.into(), left.into(), right.into()])
            }
            Op::Mod { dest, left, right } => {
                self.write(OpCode::Mod, &[dest.into(), left.into(), right.into()])
            }
            Op::Pow { dest, left, right } => {
                self.write(OpCode::Pow, &[dest.into(), left.into(), right.into()])
            }
            Op::Concat { dest, left, right } => {
                self.write(OpCode::Concat, &[dest.into(), left.into(), right.into()])
            }
            Op::Eq { dest, left, right } => {
                self.write(OpCode::Eq, &[dest.into(), left.into(), right.into()])
            }
            Op::Ne { dest, left, right } => {
                self.write(OpCode::Ne, &[dest.into(), left.into(), right.into()])
            }
            Op::Lt { dest, left, right } => {
                self.write(OpCode::Lt, &[dest.into(), left.into(), right.into()])
            }
            Op::Le { dest, left, right } => {
                self.write(OpCode::Le, &[dest.into(), left.into(), right.into()])
            }
            Op::Gt { dest, left, right } => {
                self.write(OpCode::Gt, &[dest.into(), left.into(), right.into()])
            }
            Op::Ge { dest, left, right } => {
                self.write(OpCode::Ge, &[dest.into(), left.into(), right.into()])
            }
            Op::BAnd { dest, left, right } => {
                self.write(OpCode::BAnd, &[dest.into(), left.into(), right.into()])
            }
            Op::BOr { dest, left, right } => {
                self.write(OpCode::BOr, &[dest.into(), left.into(), right.into()])
            }
            Op::BXor { dest, left, right } => {
                self.write(OpCode::BXor, &[dest.into(), left.into(), right.into()])
            }
            Op::Shl { dest, left, right } => {
                self.write(OpCode::Shl, &[dest.into(), left.into(), right.into()])
            }
            Op::Shr { dest, left, right } => {
                self.write(OpCode::Shr, &[dest.into(), left.into(), right.into()])
            }
            Op::Call {
                callee,
                args,
                results,
            } => self.write(OpCode::Call, &[callee.into(), args.into(), results.into()]),
            Op::TailCall { callee, args } => {
                self.write(OpCode::TailCall, &[callee.into(), args.into()])
            }
            Op::Return { first, count } => {
                self.write(OpCode::Return, &[first.into(), count.into()])
            }
            Op::Jump { offset } => self.write(OpCode::Jump, &[offset as u32]),
            Op::JumpIfTrue { cond, offset } => {
                self.write(OpCode::JumpIfTrue, &[cond.into(), offset as u32])
            }
            Op::JumpIfFalse { cond, offset } => {
                self.write(OpCode::JumpIfFalse, &[cond.into(), offset as u32])
            }
            Op::ForPrep { control, offset } => {
                self.write(OpCode::ForPrep, &[control.into(), offset as u32])
            }
            Op::ForLoop { control, offset } => {
                self.write(OpCode::ForLoop, &[control.into(), offset as u32])
            }
        }

        at
    }

    /// Rewrites the displacement of the jump beginning at `at` so that control lands on `target`.
    pub fn patch_jump(&mut self, at: u32, target: u32) {
        let byte = self.code[at as usize];

        debug_assert!(matches!(
            OpCode::from_byte(byte),
            Some(
                OpCode::Jump
                    | OpCode::JumpIfTrue
                    | OpCode::JumpIfFalse
                    | OpCode::ForPrep
                    | OpCode::ForLoop
            )
        ));

        let next = at + instruction_len(byte).expect("an emitted opcode");
        let offset =
            i32::try_from(i64::from(target) - i64::from(next)).expect("a displacement in range");
        let slot = (next - 4) as usize;

        self.code[slot..slot + 4].copy_from_slice(&offset.to_le_bytes());
    }

    fn write(&mut self, opcode: OpCode, operands: &[u32]) {
        debug_assert_eq!(operands.len(), opcode.operands().len());

        self.code.push(opcode as u8);

        for (raw, kind) in operands.iter().zip(opcode.operands()) {
            let bytes = raw.to_le_bytes();
            self.code.extend_from_slice(&bytes[..kind.width() as usize]);
        }
    }
}
