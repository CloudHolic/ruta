//! Lowering virtual registers to the ones a frame holds.

mod assign;
mod frame;
mod live;
mod order;
mod window;

pub use frame::allocate;
