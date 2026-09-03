//! Lua tables.

mod access;
mod key;
mod store;

pub use key::{InvalidKey, KeyError};

pub(in crate::heap) use store::Table;
