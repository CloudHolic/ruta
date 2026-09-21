//! The machine: one heap, one pair of statcks, one table of globals.

use ruta_bytecode::Prototype;

use crate::heap::{Function, Heap, ProtoRef, StrRef, TableRef, UpvalRef, Upvalue};
use crate::stack::{Frame, Stack};
use crate::value::Value;

use super::base;
use super::call;
use super::dispatch;
use super::error::{Error, chunk};

#[derive(Debug)]
pub struct Vm {
    pub(super) heap: Heap,
    pub(super) stack: Stack,
    pub(super) globals: TableRef,
    /// The upvalues still pointing into the stack, ordered by slot.
    pub(super) open: Vec<(u32, UpvalRef)>,
}

impl Vm {
    pub fn new() -> Vm {
        let mut heap = Heap::default();
        let globals = heap.new_table(0, 0);
        let mut vm = Vm {
            heap,
            stack: Stack::default(),
            globals,
            open: Vec::new(),
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

    /// Raises a runtime error at the instruciton now running.
    pub(super) fn throw(&mut self, message: String) -> Error {
        let mut text = chunk(self.source());
        text.extend_from_slice(format!(":{}: {message}", self.line()).as_bytes());

        let handle = self.heap.new_string(&text);

        Error {
            value: Value::Str(handle),
        }
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

    /// Moves the value out of every slot from `from` up into its cell.
    pub(super) fn close(&mut self, from: u32) {
        let at = self.open.partition_point(|(slot, _)| *slot < from);

        for (slot, cell) in self.open.split_off(at) {
            let value = self.stack.at(slot);
            self.heap.set_upvalue(cell, Upvalue::Closed(value));
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
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
