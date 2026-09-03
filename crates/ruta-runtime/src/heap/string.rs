//! Lua strings and the table that interns the short ones.

use std::mem;

use super::arena::Heap;
use super::handle::StrRef;

const SHORT_STRING_MAX: usize = 40;
const INTERN_INITIAL_BUCKETS: usize = 16;

/// A Lua string: bytes plus a trailing NUL that is not part of the value.
#[derive(Debug)]
pub struct LuaStr {
    storage: Box<[u8]>,
    hash: u32,
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

    pub(super) fn hash(&self) -> u32 {
        self.hash
    }
}

impl Heap {
    /// Puts a string on the heap. Strings of `SHORT_STRING_MAX` bytes or fewer are interned.
    pub fn new_string(&mut self, bytes: &[u8]) -> StrRef {
        let short = bytes.len() <= SHORT_STRING_MAX;
        let hash = hash_bytes(bytes);

        if short && let Some(existing) = self.find_interned(bytes, hash) {
            return existing;
        }

        let mut storage = Vec::with_capacity(bytes.len() + 1);
        storage.extend_from_slice(bytes);
        storage.push(0);

        let handle = self.push_string(LuaStr {
            storage: storage.into_boxed_slice(),
            hash,
        });

        if short {
            self.intern(handle, hash);
        }

        handle
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

pub(super) fn hash_bytes(bytes: &[u8]) -> u32 {
    let mut hash = hash_seed();

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    // The table indexes with the low bits.
    ((hash >> 32) as u32) ^ (hash as u32)
}

fn hash_seed() -> u64 {
    0xcbf2_9ce4_8422_2325
}
