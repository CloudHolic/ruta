//! The arena itself: the slots, and the one place they are written to.

use std::mem;

use super::function::{Function, Upvalue};
use super::handle::{FuncRef, ProtoRef, StrRef, TableRef, UpvalRef};
use super::proto::Proto;
use super::string::LuaStr;
use super::table::Table;

/// Every object ruta allocates.
#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Object>,
    /// Short strings by content.
    pub(super) interned: Vec<Option<StrRef>>,
    pub(super) interned_len: usize,
}

/// What a slot holds.
#[derive(Debug)]
enum Object {
    Str(LuaStr),
    Table(Table),
    Proto(Proto),
    Func(Function),
    Upval(Upvalue),
}

impl Heap {
    /// How many objects the heap holds.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn new_table(&mut self, array_hint: usize, hash_hint: usize) -> TableRef {
        self.objects
            .push(Object::Table(Table::with_hints(array_hint, hash_hint)));

        TableRef(self.objects.len() as u32 - 1)
    }

    pub fn string(&self, handle: StrRef) -> &LuaStr {
        match &self.objects[handle.0 as usize] {
            Object::Str(string) => string,
            other => panic!("StrRef names {other:?}"),
        }
    }

    pub fn proto(&self, handle: ProtoRef) -> &Proto {
        match &self.objects[handle.0 as usize] {
            Object::Proto(proto) => proto,
            other => panic!("ProtoRef names {other:?}"),
        }
    }

    pub fn func(&self, handle: FuncRef) -> &Function {
        match &self.objects[handle.0 as usize] {
            Object::Func(function) => function,
            other => panic!("FuncRef names {other:?}"),
        }
    }

    pub fn upvalue(&self, handle: UpvalRef) -> Upvalue {
        match &self.objects[handle.0 as usize] {
            Object::Upval(cell) => *cell,
            other => panic!("UpvalRef names {other:?}"),
        }
    }

    /// Puts a string in a slot.
    pub(super) fn push_string(&mut self, string: LuaStr) -> StrRef {
        self.objects.push(Object::Str(string));

        StrRef(self.objects.len() as u32 - 1)
    }

    pub(super) fn push_proto(&mut self, proto: Proto) -> ProtoRef {
        self.objects.push(Object::Proto(proto));

        ProtoRef(self.objects.len() as u32 - 1)
    }

    pub(super) fn push_func(&mut self, function: Function) -> FuncRef {
        self.objects.push(Object::Func(function));

        FuncRef(self.objects.len() as u32 - 1)
    }

    pub(super) fn push_upvalue(&mut self, cell: Upvalue) -> UpvalRef {
        self.objects.push(Object::Upval(cell));

        UpvalRef(self.objects.len() as u32 - 1)
    }

    pub(super) fn put_upvalue(&mut self, handle: UpvalRef, cell: Upvalue) {
        match &mut self.objects[handle.0 as usize] {
            Object::Upval(held) => *held = cell,
            other => panic!("UpvalRef names {other:?}"),
        }
    }

    /// Every write to a heap object passes here first.
    pub(super) fn barrier(&mut self, _object: u32) {}

    pub(super) fn table(&self, handle: TableRef) -> &Table {
        match &self.objects[handle.0 as usize] {
            Object::Table(table) => table,
            other => panic!("TableRef names {other:?}"),
        }
    }

    /// Takes a table's body out, leaving an empty one behind.
    pub(super) fn take_table(&mut self, handle: TableRef) -> Table {
        match &mut self.objects[handle.0 as usize] {
            Object::Table(table) => mem::take(table),
            other => panic!("TableRef names {other:?}"),
        }
    }

    pub(super) fn put_table(&mut self, handle: TableRef, body: Table) {
        match &mut self.objects[handle.0 as usize] {
            Object::Table(table) => *table = body,
            other => panic!("TableRef names {other:?}"),
        }
    }
}
