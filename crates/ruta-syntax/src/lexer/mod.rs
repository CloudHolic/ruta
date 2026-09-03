//! The lexer.

mod bytes;
mod cursor;
mod long;
mod number;
mod scan;
mod string;

pub(crate) use cursor::Lexer;
