//! Callables, and the cells a closure shares with the frame that made it.

use crate::value::Value;
use crate::vm::{Error, Vm};

use super::arena::Heap;
use super::handle::{FuncRef, ProtoRef, StrRef, UpvalRef};

/// A function the host wrote.
pub type Native = fn(&mut Vm, callee: u32, args: u16) -> Result<u16, Error>;

/// One callable. Both kinds answer `"function"`, so one handle names either.
#[derive(Debug)]
pub enum Function {
    Lua {
        proto: ProtoRef,
        upvals: Box<[UpvalRef]>,
    },
    Native {
        name: StrRef,
        call: Native,
    },
}

/// A captured local. It points at the stack slot until that slot goes away.
#[derive(Debug, Clone, Copy)]
pub enum Upvalue {
    Open(u32),
    Closed(Value),
}

impl Heap {
    pub fn new_closure(&mut self, proto: ProtoRef, upvals: Box<[UpvalRef]>) -> FuncRef {
        self.push_func(Function::Lua { proto, upvals })
    }

    pub fn new_native(&mut self, name: &[u8], call: Native) -> FuncRef {
        let name = self.new_string(name);
        self.push_func(Function::Native { name, call })
    }

    pub fn new_upvalue(&mut self, cell: Upvalue) -> UpvalRef {
        self.push_upvalue(cell)
    }

    pub fn set_upvalue(&mut self, handle: UpvalRef, cell: Upvalue) {
        self.barrier(handle.0);
        self.put_upvalue(handle, cell);
    }
}
