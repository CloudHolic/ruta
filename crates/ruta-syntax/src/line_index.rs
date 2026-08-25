//! Turning a byte offset into a line number, once per source rather than oce per token.

/// Where each line starts, so that a byte offset can be turned into a line number without every token carrying one.
#[derive(Debug)]
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(source: &[u8]) -> Self {
        let mut starts = vec![0];
        let mut at = 0;

        while at < source.len() {
            let byte = source[at];
            at += 1;
            if byte == b'\n' || byte == b'\r' {
                if source
                    .get(at)
                    .is_some_and(|next| (*next == b'\n' || *next == b'\r') && *next != byte)
                {
                    at += 1;
                }

                starts.push(at as u32);
            }
        }

        Self { starts }
    }

    /// The 1-based line holding `offset`.
    pub fn line_of(&self, offset: u32) -> u32 {
        match self.starts.binary_search(&offset) {
            Ok(index) => index as u32 + 1,
            Err(index) => index as u32,
        }
    }
}
