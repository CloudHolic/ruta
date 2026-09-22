//! Where the value in a register came from, told the way an error message names it.

use ruta_bytecode::{MULTI, Op, decode, instruction_len};

use crate::stack::Frame;
use crate::value::Value;

use super::state::Vm;

/// What a key register holds, as far as naming a field goes.
enum Key {
    Name(Vec<u8>),
    /// A small non-negative integer written into the code, which is all the message names.
    Small,
    Other,
}

enum Origin {
    Local(Vec<u8>),
    Upvalue(Vec<u8>),
    Constant(Vec<u8>),
    Global(Vec<u8>),
    Field(Vec<u8>),
    Method(Vec<u8>),
}

impl Origin {
    fn parts(self) -> (&'static str, Vec<u8>) {
        match self {
            Origin::Local(name) => ("local", name),
            Origin::Upvalue(name) => ("upvalue", name),
            Origin::Constant(name) => ("constant", name),
            Origin::Global(name) => ("global", name),
            Origin::Field(name) => ("field", name),
            Origin::Method(name) => ("method", name),
        }
    }

    fn render(self) -> Vec<u8> {
        let (kind, name) = self.parts();
        let mut out = format!("{kind} '").into_bytes();
        out.extend_from_slice(&name);
        out.push(b'\'');

        out
    }

    fn name(self) -> Vec<u8> {
        self.parts().1
    }
}

/// The running function's instructions, and every place a jump lands.
struct Code {
    ops: Vec<(u32, Op)>,
    jumps: Vec<(u32, u32)>,
}

impl Code {
    fn of(vm: &Vm) -> Code {
        let code = &vm.heap.proto(vm.running()).code;
        let mut ops = Vec::new();
        let mut jumps = Vec::new();
        let mut pc = 0u32;

        while (pc as usize) < code.len() {
            let (op, len) = decode(code, pc).expect("code the emitter wrote");

            if let Op::Jump { offset }
            | Op::JumpIfTrue { offset, .. }
            | Op::JumpIfFalse { offset, .. }
            | Op::ForPrep { offset, .. }
            | Op::ForLoop { offset, .. } = op
            {
                let next = pc + instruction_len(code[pc as usize]).expect("a known opcode");
                jumps.push((pc, next.wrapping_add_signed(offset)));
            }

            ops.push((pc, op));
            pc += len;
        }

        Code { ops, jumps }
    }

    /// Where the instruction now running begins. The frame's pc is already past it.
    fn current(&self, vm: &Vm) -> u32 {
        let Frame::Lua { pc, .. } = *vm.stack.current();

        self.ops
            .iter()
            .take_while(|(start, _)| *start < pc)
            .last()
            .map_or(0, |(start, _)| *start)
    }

    fn setter(&self, reg: u8, at: u32) -> Option<(u32, Op)> {
        let (pc, op) = *self
            .ops
            .iter()
            .take_while(|(start, _)| *start < at)
            .filter(|(_, op)| writes(op, reg))
            .last()?;

        if self
            .jumps
            .iter()
            .any(|&(from, target)| from < pc && pc < target && target <= at)
        {
            return None;
        }

        Some((pc, op))
    }
}

/// The clause an error message ends with, or nothing when the origin is not known.
pub(super) fn clause(found: Option<Vec<u8>>) -> Vec<u8> {
    match found {
        Some(name) => {
            let mut out = b" (".to_vec();
            out.extend_from_slice(&name);
            out.push(b')');

            out
        }
        None => Vec::new(),
    }
}

/// Names the value an operand register holds at the instruciton now running.
pub(super) fn name(vm: &Vm, reg: u8) -> Option<Vec<u8>> {
    let code = Code::of(vm);

    trace(vm, &code, reg, code.current(vm)).map(Origin::render)
}

/// Like [`name`], for the function a call is about to make, which may have been a method.
pub(super) fn callee(vm: &Vm, reg: u8) -> Option<Vec<u8>> {
    let code = Code::of(vm);
    let at = code.current(vm);

    method(vm, &code, reg, at)
        .or_else(|| trace(vm, &code, reg, at))
        .map(Origin::render)
}

/// Just the name a call uesd, for `bad argument #n to 'name'`.
pub(super) fn called(vm: &Vm, reg: u8) -> Option<Vec<u8>> {
    let code = Code::of(vm);
    let at = code.current(vm);

    method(vm, &code, reg, at)
        .or_else(|| trace(vm, &code, reg, at))
        .map(Origin::name)
}

fn trace(vm: &Vm, code: &Code, reg: u8, at: u32) -> Option<Origin> {
    let proto = vm.heap.proto(vm.running());

    if let Some(name) = local(vm, reg, at) {
        return Some(Origin::Local(name));
    }

    let (pc, op) = code.setter(reg, at)?;

    match op {
        Op::Move { src, .. } => trace(vm, code, src, pc),
        Op::GetUpval { index, .. } => Some(Origin::Upvalue(
            proto.upvals[usize::from(index)].name.to_vec(),
        )),
        Op::LoadConst { constant, .. } => match proto.constants[constant as usize] {
            Value::Str(handle) => Some(Origin::Constant(vm.bytes(handle).to_vec())),
            _ => None,
        },
        Op::Index { object, key, .. } => Some(match key_of(vm, code, key, pc) {
            Key::Name(name) if env(vm, code, object, pc) => Origin::Global(name),
            Key::Name(name) => Origin::Field(name),
            Key::Small => Origin::Field(b"integer index".to_vec()),
            Key::Other => Origin::Field(b"?".to_vec()),
        }),
        _ => None,
    }
}

/// `object:name(...)` reads `object` once and hands it to the call as its first argument,
/// which `object.name(object)` does not: that one reads `object` twice.
fn method(vm: &Vm, code: &Code, reg: u8, at: u32) -> Option<Origin> {
    let (index_at, Op::Index { object, key, .. }) = code.setter(reg, at)? else {
        return None;
    };
    let Key::Name(name) = key_of(vm, code, key, index_at) else {
        return None;
    };
    let (_, Op::Move { src, .. }) = code.setter(reg.checked_add(1)?, at)? else {
        return None;
    };

    (src == object).then_some(Origin::Method(name))
}

fn local(vm: &Vm, reg: u8, at: u32) -> Option<Vec<u8>> {
    vm.heap
        .proto(vm.running())
        .locals
        .iter()
        .rev()
        .find(|local| local.register == reg && local.start_pc <= at && at < local.end_pc)
        .map(|local| local.name.to_vec())
}

fn key_of(vm: &Vm, code: &Code, key: u8, at: u32) -> Key {
    if local(vm, key, at).is_some() {
        return Key::Other;
    }

    let Some((_, Op::LoadConst { constant, .. })) = code.setter(key, at) else {
        return Key::Other;
    };

    match vm.heap.proto(vm.running()).constants[constant as usize] {
        Value::Str(handle) => Key::Name(vm.bytes(handle).to_vec()),
        Value::Int(number) if (0..=255).contains(&number) => Key::Small,
        _ => Key::Other,
    }
}

/// A table read through `_ENV` is a global, whether `_ENV` is the chunk's upvalue or a local
/// that shadows it.
fn env(vm: &Vm, code: &Code, object: u8, at: u32) -> bool {
    matches!(
        trace(vm, code, object, at),
        Some(Origin::Local(name) | Origin::Upvalue(name)) if name == b"_ENV"
    )
}

fn writes(op: &Op, reg: u8) -> bool {
    let reg = u16::from(reg);

    match *op {
        Op::LoadNil { dest }
        | Op::LoadTrue { dest }
        | Op::LoadFalse { dest }
        | Op::LoadConst { dest, .. }
        | Op::Move { dest, .. }
        | Op::GetUpval { dest, .. }
        | Op::Closure { dest, .. }
        | Op::NewTable { dest, .. }
        | Op::Index { dest, .. }
        | Op::Neg { dest, .. }
        | Op::Not { dest, .. }
        | Op::Len { dest, .. }
        | Op::BNot { dest, .. }
        | Op::Add { dest, .. }
        | Op::Sub { dest, .. }
        | Op::Mul { dest, .. }
        | Op::Div { dest, .. }
        | Op::IDiv { dest, .. }
        | Op::Mod { dest, .. }
        | Op::Pow { dest, .. }
        | Op::Concat { dest, .. }
        | Op::Eq { dest, .. }
        | Op::Ne { dest, .. }
        | Op::Lt { dest, .. }
        | Op::Le { dest, .. }
        | Op::Gt { dest, .. }
        | Op::Ge { dest, .. }
        | Op::BAnd { dest, .. }
        | Op::BOr { dest, .. }
        | Op::BXor { dest, .. }
        | Op::Shl { dest, .. }
        | Op::Shr { dest, .. } => u16::from(dest) == reg,
        Op::Vararg { first, count } => {
            reg >= u16::from(first) && (count == MULTI || reg < u16::from(first) + u16::from(count))
        }
        // The results land from the callee up, and the callee's own frame above them.
        Op::Call { callee, .. } => reg >= u16::from(callee),
        Op::ForPrep { control, .. } | Op::ForLoop { control, .. } => {
            (u16::from(control)..u16::from(control) + 4).contains(&reg)
        }
        _ => false,
    }
}
