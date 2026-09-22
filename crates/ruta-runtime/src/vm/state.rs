//! The machine: one heap, one pair of statcks, one table of globals.

use ruta_bytecode::Prototype;
use ruta_syntax::{Number, parse_number};

use crate::heap::{FuncRef, Function, Heap, ProtoRef, StrRef, TableRef, UpvalRef, Upvalue};
use crate::stack::{Frame, Stack};
use crate::value::Value;

use super::base;
use super::call;
use super::dispatch;
use super::error::{Error, chunk};
use super::origin;

#[derive(Debug)]
pub struct Vm {
    pub(super) heap: Heap,
    pub(super) stack: Stack,
    pub(super) globals: TableRef,
    /// The upvalues still pointing into the stack, ordered by slot.
    pub(super) open: Vec<(u32, UpvalRef)>,
    /// The iterators `pairs` and `ipairs` hand out, made once so that each is always the smae function:
    /// `pairs(t) == next`.
    pub(super) next: FuncRef,
    pub(super) ipairs_step: FuncRef,
}

impl Vm {
    pub fn new() -> Vm {
        let mut heap = Heap::default();
        let globals = heap.new_table(0, 0);
        let (next, ipairs_step) = base::iterators(&mut heap);
        let mut vm = Vm {
            heap,
            stack: Stack::default(),
            globals,
            open: Vec::new(),
            next,
            ipairs_step,
        };

        base::install(&mut vm);

        vm
    }

    pub fn run(&mut self, prototype: Prototype) -> Result<(), Error> {
        let proto = self.heap.load(prototype);
        let env = self
            .heap
            .new_upvalue(Upvalue::Closed(Value::Table(self.globals)));
        let main = self.heap.new_closure(proto, Box::new([env]));

        let at = self.stack.height();
        self.stack.reserve(at + 1);
        self.stack.put(at, Value::Func(main));

        call::enter(self, at, 0, crate::stack::Want::Exactly(0))?;

        dispatch::run(self)
    }

    /// The bytes `tostring` would answer with.
    pub fn text(&mut self, value: Value) -> Vec<u8> {
        super::text::of(self, value)
    }

    /// What the command line shows for an error nothing caught.
    pub fn message(&mut self, error: &Error) -> Vec<u8> {
        match error.value {
            Value::Str(_) | Value::Int(_) | Value::Float(_) => self.text(error.value),
            other => format!("(error object is a {} value)", other.type_name()).into_bytes(),
        }
    }

    /// Raises a runtime error at the instruciton now running.
    pub(super) fn throw(&mut self, message: impl AsRef<[u8]>) -> Error {
        let mut text = chunk(self.source());
        text.extend_from_slice(format!(":{}: ", self.line()).as_bytes());
        text.extend_from_slice(message.as_ref());

        let handle = self.heap.new_string(&text);

        Error {
            value: Value::Str(handle),
        }
    }

    /// `attempt to <what> a <type> value`, naming where the value came from when that is known.
    pub(super) fn fault(&mut self, what: &str, value: Value, reg: u8) -> Error {
        let mut message = format!("attempt to {what} a {} value", value.type_name()).into_bytes();
        message.extend(origin::clause(origin::name(self, reg)));

        self.throw(message)
    }

    pub(super) fn fault_call(&mut self, value: Value, callee: u32) -> Error {
        let Frame::Lua { base, .. } = *self.stack.current();
        let reg = u8::try_from(callee - base).expect("a register of the running frame");

        let mut message = format!("attempt to call a {} value", value.type_name()).into_bytes();
        message.extend(origin::clause(origin::callee(self, reg)));

        self.throw(message)
    }

    pub(super) fn running(&self) -> ProtoRef {
        let Frame::Lua { func, .. } = *self.stack.current();
        let Function::Lua { proto, .. } = self.heap.func(func) else {
            unreachable!("a Lua frame holds a Lua closure");
        };

        *proto
    }

    pub(super) fn cell(&self, index: usize) -> UpvalRef {
        let Frame::Lua { func, .. } = *self.stack.current();
        let Function::Lua { upvals, .. } = self.heap.func(func) else {
            unreachable!("a Lua frame holds a Lua closure");
        };

        upvals[index]
    }

    pub(super) fn bytes(&self, handle: StrRef) -> &[u8] {
        self.heap.string(handle).as_bytes()
    }

    pub(super) fn intern(&mut self, bytes: &[u8]) -> StrRef {
        self.heap.new_string(bytes)
    }

    pub(super) fn constant(&self, index: u32) -> Value {
        self.heap.proto(self.running()).constants[index as usize]
    }

    pub(super) fn upvalue(&self, index: usize) -> Upvalue {
        self.heap.upvalue(self.cell(index))
    }

    /// The cell for a stack slot, shared by every closure that captures the same one.
    pub(super) fn capture(&mut self, slot: u32) -> UpvalRef {
        match self.open.binary_search_by_key(&slot, |(held, _)| *held) {
            Ok(at) => self.open[at].1,
            Err(at) => {
                let cell = self.heap.new_upvalue(Upvalue::Open(slot));
                self.open.insert(at, (slot, cell));

                cell
            }
        }
    }

    /// Where the row a multiple-result instruction left ends.
    pub(super) fn pending(&self) -> u32 {
        let Frame::Lua { top, .. } = *self.stack.current();

        top
    }

    /// The row has been consumeed, so the frame's top goes back to its registers.
    pub(super) fn settle_top(&mut self) {
        let registers = u32::from(self.heap.proto(self.running()).max_registers);
        let Frame::Lua { base, top, .. } = self.stack.current_mut();

        *top = *base + registers;
    }

    /// The number a value is or spells out.
    pub(super) fn to_number(&self, value: Value) -> Option<Value> {
        match value {
            Value::Int(_) | Value::Float(_) => Some(value),
            Value::Str(handle) => parse_number(self.bytes(handle)).map(|number| match number {
                Number::Int(number) => Value::Int(number),
                Number::Float(number) => Value::Float(number),
            }),
            _ => None,
        }
    }

    /// `bad argument #n` to 'name' (problem)`.
    pub(super) fn bad_argument(&mut self, callee: u32, index: u32, problem: &str) -> Error {
        let Frame::Lua { base, .. } = *self.stack.current();
        let name = u8::try_from(callee - base)
            .ok()
            .and_then(|reg| origin::called(self, reg))
            .unwrap_or_else(|| self.registered(callee));

        let mut message = format!("bad argument #{index} to '").into_bytes();
        message.extend_from_slice(&name);
        message.extend_from_slice(format!("' ({problem})").as_bytes());

        self.throw(message)
    }

    /// Moves the value out of every slot from `from` up into its cell.
    pub(super) fn close(&mut self, from: u32) {
        let at = self.open.partition_point(|(slot, _)| *slot < from);

        for (slot, cell) in self.open.split_off(at) {
            let value = self.stack.at(slot);
            self.heap.set_upvalue(cell, Upvalue::Closed(value));
        }
    }

    /// An error a host funciton raises from inside itself, which points at no line.
    pub(super) fn bare(&mut self, message: impl AsRef<[u8]>) -> Error {
        let handle = self.heap.new_string(message.as_ref());

        Error {
            value: Value::Str(handle),
        }
    }

    fn source(&self) -> &[u8] {
        let proto = self.heap.proto(self.running());

        self.heap.string(proto.source).as_bytes()
    }

    fn line(&self) -> u32 {
        let Frame::Lua { pc, .. } = *self.stack.current();

        self.heap.proto(self.running()).lines.line_at(pc - 1)
    }

    fn registered(&self, callee: u32) -> Vec<u8> {
        match self.stack.at(callee) {
            Value::Func(handle) => match self.heap.func(handle) {
                Function::Native { name, .. } => self.bytes(*name).to_vec(),
                Function::Lua { .. } => b"?".to_vec(),
            },
            _ => b"?".to_vec(),
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
