//! Call frames.

use crate::heap::FuncRef;
use crate::value::Value;

/// How many values the caller of a frame expects back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    Exactly(u16),
    /// However many the callee returns.
    All,
}

#[derive(Debug)]
pub enum Frame {
    Lua {
        func: FuncRef,
        /// Index into the value stack, not a pointer: the stack reallocates as it grows.
        base: u32,
        top: u32,
        pc: u32,
        want: Want,
        /// Where the results go in the caller.
        ret_to: u32,
    },
}

impl Frame {
    pub fn roots(&self, values: &[Value], visit: &mut dyn FnMut(Value)) {
        match *self {
            Frame::Lua { base, top, .. } => {
                for value in &values[base as usize..top as usize] {
                    visit(*value);
                }
            }
        }
    }
}
