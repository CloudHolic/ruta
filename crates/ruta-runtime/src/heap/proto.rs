//! A compiled function as the runtime holds it, and the path a chunk takes onto the heap.

use ruta_bytecode::{Constant, LineTable, LocalVar, Prototype, UpvalDesc, Vararg};

use crate::value::Value;

use super::arena::Heap;
use super::handle::{ProtoRef, StrRef};

/// One prototype on the heap. The constants are resolved and the children are handles,
/// so nothing here reaches bakc into the tree the compiler built.
#[derive(Debug)]
pub struct Proto {
    pub params: u8,
    pub vararg: Vararg,
    pub max_registers: u8,
    pub code: Box<[u8]>,
    pub constants: Box<[Value]>,
    /// Indexed the way a closure instruction names one.
    pub children: Box<[ProtoRef]>,
    pub upvals: Box<[UpvalDesc]>,
    pub source: StrRef,
    pub line_defined: u32,
    pub last_line_defined: u32,
    pub lines: LineTable,
    pub locals: Box<[LocalVar]>,
}

impl Heap {
    /// Puts a compiled chunk on the heap, one object per prototype, and answers the root.
    pub fn load(&mut self, prototype: Prototype) -> ProtoRef {
        let source = self.new_string(&prototype.source);
        self.load_child(prototype, source)
    }

    fn load_child(&mut self, prototype: Prototype, source: StrRef) -> ProtoRef {
        let children = prototype
            .children
            .into_vec()
            .into_iter()
            .map(|child| self.load_child(child, source))
            .collect();

        let constants = prototype
            .constants
            .into_vec()
            .into_iter()
            .map(|constant| self.resolve(constant))
            .collect();

        self.push_proto(Proto {
            params: prototype.params,
            vararg: prototype.vararg,
            max_registers: prototype.max_registers,
            code: prototype.code,
            constants,
            children,
            upvals: prototype.upvals,
            source,
            line_defined: prototype.line_defined,
            last_line_defined: prototype.last_line_defined,
            lines: prototype.lines,
            locals: prototype.locals,
        })
    }

    fn resolve(&mut self, constant: Constant) -> Value {
        match constant {
            Constant::Int(number) => Value::Int(number),
            Constant::Float(number) => Value::Float(number),
            Constant::Str(bytes) => Value::Str(self.new_string(&bytes)),
        }
    }
}
