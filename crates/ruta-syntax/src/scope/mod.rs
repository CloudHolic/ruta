//! Name resolution, and the assignments it refuses.

mod binding;
mod expr;
mod label;
mod resolver;
mod stat;

pub use binding::{Access, Binding, Bindings, Capture, FunctionBindings};
pub use resolver::resolve;
