//! The machine: one heap, one pair of statcks, one table of globals.

use ruta_bytecode::Prototype;

use crate::heap::{Function, Heap, TableRef, Upvalue};
use crate::stack::{Frame, Stack};
use crate::value::Value;

use super::base;
use super::call;
use super::dispatch;
use super::error::Error;

#[derive(Debug)]
pub struct Vm {
    pub(super) heap: Heap,
    pub(super) stack: Stack,
    pub(super) globals: TableRef,
}

impl Vm {
    pub fn new() -> Vm {
        let mut heap = Heap::default();
        let globals = heap.new_table(0, 0);
        let mut vm = Vm {
            heap,
            stack: Stack::default(),
            globals,
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

    pub(super) fn constant(&self, index: u32) -> Value {
        let Frame::Lua { func, .. } = *self.stack.current();
        let Function::Lua { proto, .. } = self.heap.func(func) else {
            unreachable!("a Lua frame holds a Lua closure");
        };

        self.heap.proto(*proto).constants[index as usize]
    }

    pub(super) fn upvalue(&self, index: usize) -> Upvalue {
        let Frame::Lua { func, .. } = *self.stack.current();
        let Function::Lua { upvals, .. } = self.heap.func(func) else {
            unreachable!("a Lua frame holds a Lua closure");
        };

        self.heap.upvalue(upvals[index])
    }

    pub(super) fn throw(&mut self, message: String) -> Error {
        let handle = self.heap.new_string(message.as_bytes());

        Error {
            value: Value::Str(handle),
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
