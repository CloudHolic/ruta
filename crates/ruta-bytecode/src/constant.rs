//! The constant pool.

/// A literal the code generator cannot turn into a heap object.
/// The runtime resolves these when it loads a chunk, interning the strings.
#[derive(Debug)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Str(Box<[u8]>),
}
