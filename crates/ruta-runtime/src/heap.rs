//! The heap: one arena, and handles that index it.

use std::mem;

const SHORT_STRING_MAX: usize = 40;
const INTERN_INITIAL_BUCKETS: usize = 16;

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
    /// Short strings by content.
    interned: Vec<Option<StrRef>>,
    interned_len: usize,
}

impl Heap {
    /// Puts a string on the heap. Strings of `SHORT_STRING_MAX` bytes or fewer are interned.
    pub fn new_string(&mut self, bytes: &[u8]) -> StrRef {
        let short = bytes.len() <= SHORT_STRING_MAX;
        let hash = if short { hash_bytes(bytes) } else { 0 };

        if short && let Some(existing) = self.find_interned(bytes, hash) {
            return existing;
        }

        let mut storage = Vec::with_capacity(bytes.len() + 1);
        storage.extend_from_slice(bytes);
        storage.push(0);

        self.objects.push(Object::Str(LuaStr {
            storage: storage.into_boxed_slice(),
        }));

        let handle = StrRef(self.objects.len() as u32 - 1);

        if short {
            self.intern(handle, hash);
        }

        handle
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

    /// The handle of an interned string with these bytes, if there is one.
    fn find_interned(&self, bytes: &[u8], hash: u32) -> Option<StrRef> {
        if self.interned.is_empty() {
            return None;
        }

        let mask = self.interned.len() - 1;
        let mut index = hash as usize & mask;

        // Linear probing, and the table is never full.
        loop {
            match self.interned[index] {
                None => return None,
                Some(handle) if self.string(handle).as_bytes() == bytes => return Some(handle),
                Some(_) => index = (index + 1) & mask,
            }
        }
    }

    /// Puts an already-allocated short string into the intern table.
    fn intern(&mut self, handle: StrRef, hash: u32) {
        if (self.interned_len + 1) * 4 >= self.interned.len() * 3 {
            self.grow_interned();
        }

        let mask = self.interned.len() - 1;
        let mut index = hash as usize & mask;

        while self.interned[index].is_some() {
            index = (index + 1) & mask;
        }

        self.interned[index] = Some(handle);
        self.interned_len += 1;
    }

    /// Doubles the table and reinserts.
    fn grow_interned(&mut self) {
        let buckets = match self.interned.len() {
            0 => INTERN_INITIAL_BUCKETS,
            current => current * 2,
        };

        let old = mem::replace(&mut self.interned, vec![None; buckets]);
        let mask = buckets - 1;

        for handle in old.into_iter().flatten() {
            let hash = hash_bytes(self.string(handle).as_bytes());
            let mut index = hash as usize & mask;

            while self.interned[index].is_some() {
                index = (index + 1) & mask;
            }

            self.interned[index] = Some(handle);
        }
    }
}

fn hash_seed() -> u64 {
    0xcbf2_9ce4_8422_2325
}

fn hash_bytes(bytes: &[u8]) -> u32 {
    let mut hash = hash_seed();

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    // The table indexes with the low bits.
    ((hash >> 32) as u32) ^ (hash as u32)
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
    fn equal_short_strings_are_one_object() {
        let mut heap = Heap::default();
        let first = heap.new_string(b"x");
        let second = heap.new_string(b"x");

        // Interning is what lets stage 4 compare short strings by handle.
        assert_eq!(first, second);
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn the_hash_depends_on_every_byte() {
        assert_ne!(hash_bytes(b"ab"), hash_bytes(b"ba"));
        assert_ne!(hash_bytes(b"a"), hash_bytes(b"a\0"));
        assert_eq!(hash_bytes(b"hello"), hash_bytes(b"hello"));
    }

    #[test]
    fn the_hash_of_the_empty_string_is_the_seed() {
        // Folded, but still the seed and nothing else: an empty string must not collide
        // with whatever a zeroed buffer would produce.
        assert_eq!(
            hash_bytes(b""),
            ((hash_seed() >> 32) as u32) ^ (hash_seed() as u32)
        );
    }

    #[test]
    fn long_strings_are_not_interned() {
        let mut heap = Heap::default();
        let long = vec![b'x'; SHORT_STRING_MAX + 1];

        let first = heap.new_string(&long);
        let second = heap.new_string(&long);

        assert_ne!(first, second);
        assert_eq!(
            heap.string(first).as_bytes(),
            heap.string(second).as_bytes()
        );
    }

    #[test]
    fn the_boundary_itself_is_interned() {
        let mut heap = Heap::default();
        let at_limit = vec![b'x'; SHORT_STRING_MAX];

        assert_eq!(heap.new_string(&at_limit), heap.new_string(&at_limit));
    }

    #[test]
    fn interning_compares_bytes_and_not_just_hashes() {
        let mut heap = Heap::default();
        let first = heap.new_string(b"ab");
        let second = heap.new_string(b"ba");

        assert_ne!(first, second);
        assert_eq!(heap.string(first).as_bytes(), b"ab");
        assert_eq!(heap.string(second).as_bytes(), b"ba");
    }

    #[test]
    fn a_string_with_nul_interns_by_its_whole_content() {
        let mut heap = Heap::default();

        // The NUL is part of the value, so these are two different strings (invariant 2).
        assert_ne!(heap.new_string(b"a\0b"), heap.new_string(b"a"));
        assert_eq!(heap.new_string(b"a\0b"), heap.new_string(b"a\0b"));
    }

    #[test]
    fn the_table_survives_growing() {
        let mut heap = Heap::default();
        let handles: Vec<_> = (0..200u32)
            .map(|i| heap.new_string(i.to_string().as_bytes()))
            .collect();

        // Every one of them is still findable, and asking again returns the same handle.
        for (i, handle) in handles.iter().enumerate() {
            let again = heap.new_string(i.to_string().as_bytes());
            assert_eq!(*handle, again);
        }

        assert_eq!(heap.len(), 200);
    }
}
