//! Reading instructions back out of bytes.

use crate::op::{Op, OpCode, instruction_len};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The byte at this pc is not an opcode.
    UnknownOpCode { at: u32, byte: u8 },
    /// The instruction at this pc runs past the end of the code.
    Truncated { at: u32 },
}

/// Reads the instruction beginning at `pc`, returning it and how many bytes it took.
pub fn decode(code: &[u8], pc: u32) -> Result<(Op, u32), DecodeError> {
    let at = pc as usize;
    let byte = *code.get(at).ok_or(DecodeError::Truncated { at: pc })?;
    let opcode = OpCode::from_byte(byte).ok_or(DecodeError::UnknownOpCode { at: pc, byte })?;
    let len = instruction_len(byte).expect("a known opcode");

    if at + len as usize > code.len() {
        return Err(DecodeError::Truncated { at: pc });
    }

    let raw = read_operands(code, at + 1, opcode);

    let op = match opcode {
        OpCode::LoadNil => Op::LoadNil { dest: raw[0] as u8 },
        OpCode::LoadTrue => Op::LoadTrue { dest: raw[0] as u8 },
        OpCode::LoadFalse => Op::LoadFalse { dest: raw[0] as u8 },
        OpCode::LoadConst | OpCode::LoadConstWide => Op::LoadConst {
            dest: raw[0] as u8,
            constant: raw[1],
        },
        OpCode::Move => Op::Move {
            dest: raw[0] as u8,
            src: raw[1] as u8,
        },
        OpCode::GetUpval => Op::GetUpval {
            dest: raw[0] as u8,
            index: raw[1] as u8,
        },
        OpCode::SetUpval => Op::SetUpval {
            index: raw[0] as u8,
            src: raw[1] as u8,
        },
        OpCode::CloseUpvals => Op::CloseUpvals { from: raw[0] as u8 },
        OpCode::Closure => Op::Closure {
            dest: raw[0] as u8,
            child: raw[1],
        },
        OpCode::Vararg => Op::Vararg {
            first: raw[0] as u8,
            count: raw[1] as u8,
        },
        OpCode::NewTable => Op::NewTable {
            dest: raw[0] as u8,
            array_hint: raw[1],
            hash_hint: raw[2],
        },
        OpCode::Index => Op::Index {
            dest: raw[0] as u8,
            object: raw[1] as u8,
            key: raw[2] as u8,
        },
        OpCode::SetIndex => Op::SetIndex {
            object: raw[0] as u8,
            key: raw[1] as u8,
            src: raw[2] as u8,
        },
        OpCode::DefineGlobal => Op::DefineGlobal {
            env: raw[0] as u8,
            key: raw[1] as u8,
            src: raw[2] as u8,
        },
        OpCode::SetList => Op::SetList {
            table: raw[0] as u8,
            first: raw[1] as u8,
            count: raw[2] as u8,
            first_index: raw[3],
        },
        OpCode::SetListSpread => Op::SetListSpread {
            table: raw[0] as u8,
            first: raw[1] as u8,
            first_index: raw[2],
        },
        OpCode::Neg => Op::Neg {
            dest: raw[0] as u8,
            operand: raw[1] as u8,
        },
        OpCode::Not => Op::Not {
            dest: raw[0] as u8,
            operand: raw[1] as u8,
        },
        OpCode::Len => Op::Len {
            dest: raw[0] as u8,
            operand: raw[1] as u8,
        },
        OpCode::BNot => Op::BNot {
            dest: raw[0] as u8,
            operand: raw[1] as u8,
        },
        OpCode::Add => Op::Add {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Sub => Op::Sub {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Mul => Op::Mul {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Div => Op::Div {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::IDiv => Op::IDiv {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Mod => Op::Mod {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Pow => Op::Pow {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Concat => Op::Concat {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Eq => Op::Eq {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Ne => Op::Ne {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Lt => Op::Lt {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Le => Op::Le {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Gt => Op::Gt {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Ge => Op::Ge {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::BAnd => Op::BAnd {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::BOr => Op::BOr {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::BXor => Op::BXor {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Shl => Op::Shl {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Shr => Op::Shr {
            dest: raw[0] as u8,
            left: raw[1] as u8,
            right: raw[2] as u8,
        },
        OpCode::Call => Op::Call {
            callee: raw[0] as u8,
            args: raw[1] as u8,
            results: raw[2] as u8,
        },
        OpCode::TailCall => Op::TailCall {
            callee: raw[0] as u8,
            args: raw[1] as u8,
        },
        OpCode::Return => Op::Return {
            first: raw[0] as u8,
            count: raw[1] as u8,
        },
        OpCode::Jump => Op::Jump {
            offset: raw[0] as i32,
        },
        OpCode::JumpIfTrue => Op::JumpIfTrue {
            cond: raw[0] as u8,
            offset: raw[1] as i32,
        },
        OpCode::JumpIfFalse => Op::JumpIfFalse {
            cond: raw[0] as u8,
            offset: raw[1] as i32,
        },
        OpCode::ForPrep => Op::ForPrep {
            control: raw[0] as u8,
            offset: raw[1] as i32,
        },
        OpCode::ForLoop => Op::ForLoop {
            control: raw[0] as u8,
            offset: raw[1] as i32,
        },
    };

    Ok((op, len))
}

/// Reads one instruction's operands at the widths the layout gives them.
/// No insturction carries more than four.
fn read_operands(code: &[u8], mut at: usize, opcode: OpCode) -> [u32; 4] {
    debug_assert!(opcode.operands().len() <= 4);

    let mut values = [0u32; 4];

    for (slot, kind) in values.iter_mut().zip(opcode.operands()) {
        let width = kind.width() as usize;
        let mut bytes = [0u8; 4];

        bytes[..width].copy_from_slice(&code[at..at + width]);
        *slot = u32::from_le_bytes(bytes);
        at += width;
    }

    values
}
