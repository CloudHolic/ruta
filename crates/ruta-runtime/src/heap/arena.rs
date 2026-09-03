//! The arena itsef: the slots, and the one place they are written to.

use std::mem;

use super::handle::{StrRef, TableRef};
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

    /// Puts a string in a slot.
    pub(super) fn push_string(&mut self, string: LuaStr) -> StrRef {
        self.objects.push(Object::Str(string));

        StrRef(self.objects.len() as u32 - 1)
    }

    /// Every write to a heap object passes here first.
    pub(super) fn barrier(&mut self, _object: TableRef) {}

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
