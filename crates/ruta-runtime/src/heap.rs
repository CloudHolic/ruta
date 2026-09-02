//! The heap: one arena, and handles that index it.

/// Handles for each type(Str, Table, Func, UserData, and Thread).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrRef(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableRef(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncRef(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserDataRef(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadRef(u32);

/// What a slot holds.
#[derive(Debug)]
enum Object {
    Str(LuaStr),
}

/// A Lua string: bytes plus a trailing NUL that is not part of the value.
#[derive(Debug)]
pub struct LuaStr {
    storage: Box<[u8]>,
}

impl LuaStr {
    /// The bytes of the string, wihtout the trailing NUL.
    pub fn as_bytes(&self) -> &[u8] {
        &self.storage[..&self.storage.len() - 1]
    }

    pub fn len(&self) -> usize {
        self.storage.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Every object ruta allocates.
#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Object>,
}

impl Heap {
    /// Puts a string on the heap. Interning arrives with the string module; every call here allocates.
    pub fn new_string(&mut self, bytes: &[u8]) -> StrRef {
        let mut storage = Vec::with_capacity(bytes.len() + 1);
        storage.extend_from_slice(bytes);
        storage.push(0);

        self.objects.push(Object::Str(LuaStr {
            storage: storage.into_boxed_slice(),
        }));

        StrRef(self.objects.len() as u32 - 1)
    }

    pub fn string(&self, handle: StrRef) -> &LuaStr {
        match &self.objects[handle.0 as usize] {
            Object::Str(string) => string,
        }
    }

    /// How many objects the heap holds.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_string_survives_a_round_trip() {
        let mut heap = Heap::default();
        let handle = heap.new_string(b"hello");

        assert_eq!(heap.string(handle).as_bytes(), b"hello");
        assert_eq!(heap.string(handle).len(), 5);
    }

    #[test]
    fn a_string_may_contain_nul() {
        let mut heap = Heap::default();
        let handle = heap.new_string(b"a\0b");

        // The length is stored, so the embedded NUL does not end the string (invariant 2).
        assert_eq!(heap.string(handle).len(), 3);
        assert_eq!(heap.string(handle).as_bytes(), b"a\0b");
    }

    #[test]
    fn a_string_is_nul_terminated_for_the_c_abi() {
        let mut heap = Heap::default();
        let handle = heap.new_string(b"abc");
        let string = heap.string(handle);

        assert_eq!(string.storage[string.len()], 0);
    }

    #[test]
    fn the_empty_string_is_a_string() {
        let mut heap = Heap::default();
        let handle = heap.new_string(b"");

        assert!(heap.string(handle).is_empty());
        assert_eq!(heap.string(handle).as_bytes(), b"");
    }

    #[test]
    fn every_allocation_takes_a_slot() {
        let mut heap = Heap::default();
        let first = heap.new_string(b"x");
        let second = heap.new_string(b"x");

        // Two calls, two slots: nothing interns yet.
        assert_ne!(first, second);
        assert_eq!(heap.len(), 2);
    }
}
