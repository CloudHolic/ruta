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

    pub(crate) fn depth(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn enter(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    pub(crate) fn leave(&mut self) -> Option<Frame> {
        self.frames.pop()
    }

    pub(crate) fn current(&self) -> &Frame {
        self.frames.last().expect("a running frame")
    }

    pub(crate) fn current_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("a running frame")
    }

    pub(crate) fn height(&self) -> u32 {
        self.values.len() as u32
    }

    pub(crate) fn at(&self, index: u32) -> Value {
        self.values[index as usize]
    }

    pub(crate) fn put(&mut self, index: u32, value: Value) {
        self.values[index as usize] = value;
    }

    /// Make room up to `height`, filling what is new with nil.
    pub(crate) fn reserve(&mut self, height: u32) {
        if self.values.len() < height as usize {
            self.values.resize(height as usize, Value::Nil);
        }
    }

    pub(crate) fn fill(&mut self, from: u32, to: u32, value: Value) {
        for index in from..to {
            self.values[index as usize] = value;
        }
    }

    pub(crate) fn shift(&mut self, from: u32, to: u32, count: u32) {
        self.values
            .copy_within(from as usize..(from + count) as usize, to as usize);
    }
}
