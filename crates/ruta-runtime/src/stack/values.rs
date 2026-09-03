//! The two stacks, and the only thing that reads both.

use crate::value::Value;

use super::frame::Frame;

#[derive(Debug, Default)]
pub struct Stack {
    values: Vec<Value>,
    frames: Vec<Frame>,
}

impl Stack {
    /// Every value reachable from a live frame. Each frame answers for itself.
    pub fn roots(&self, visit: &mut dyn FnMut(Value)) {
        for frame in &self.frames {
            frame.roots(&self.values, visit);
        }
    }
}
