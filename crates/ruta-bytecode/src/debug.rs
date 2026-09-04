//! What a running function needs to describe itself.

/// Which source line each run of instructions came from, one entry per line change.
#[derive(Debug)]
pub struct LineTable {
    entries: Box<[(u32, u32)]>,
}

impl LineTable {
    pub fn new(entries: Box<[(u32, u32)]>) -> LineTable {
        debug_assert!(entries.windows(2).all(|pair| pair[0].0 < pair[1].0));

        LineTable { entries }
    }

    /// The line the instruction beginning at `pc` came from.
    pub fn line_at(&self, pc: u32) -> u32 {
        match self.entries.binary_search_by_key(&pc, |entry| entry.0) {
            Ok(index) => self.entries[index].1,
            Err(0) => 0,
            Err(index) => self.entries[index - 1].1,
        }
    }
}

/// A source-level local and the register it occupies for the whole of its scope.
#[derive(Debug)]
pub struct LocalVar {
    pub name: Box<[u8]>,
    pub register: u8,
    pub start_pc: u32,
    pub end_pc: u32,
}
