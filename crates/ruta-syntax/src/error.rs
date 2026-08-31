//! What the compiler refuses, and how it says so.

use crate::line_index::LineIndex;

/// What the compiler refuses a chunk for: the lexical and syntactic rules, and the semantic
/// ones the parser decides. The reference reports all of them with one status, so they share
/// one type here.
///
/// It deliberately does not implement `std::error::Error`. Rendering a message needs the
/// source's line index - `unfinished long string (starting at line 3)` carries a line number
/// inside its own text - and `Display` has nowhere to take one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Byte offset the error is reported at.
    pub at: u32,
    pub near: Near,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    MalformedNumber,
    UnfinishedString,
    HexadecimalDigitExpected,
    MissingOpenBrace,
    Utf8ValueTooLarge,
    MissingCloseBrace,
    DecimalEscapeTooLarge,
    InvalidEscapeSequence,
    InvalidLongStringDelimiter,
    UnfinishedLongString {
        open_at: u32,
    },
    UnfinishedLongComment {
        open_at: u32,
    },
    UnknownAttribute(Box<[u8]>),
    MultipleToBeClosed,
    /// `%s expected`, holding a token named the way an error message names it.
    Expected(Box<str>),
    ExpectedToClose {
        expected: Box<str>,
        open: Box<str>,
        open_at: u32,
    },
    EqualsOrInExpected,
    NameOrDotsExpected,
    FunctionArgumentsExpected,
    UnexpectedSymbol,
    VarargsOutsideVarargFunction,
    SyntaxError,
    BreakOutsideLoop,
    GlobalToBeClosed,
    ConstAssignment(Box<[u8]>),
    VariableNotDeclared(Box<[u8]>),
    EnvIsGlobal(Box<[u8]>),
    LabelAlreadyDefined {
        name: Box<[u8]>,
        /// The label that claimed the name, which the message names by line.
        first_at: u32,
    },
    NoVisibleLabel {
        name: Box<[u8]>,
        goto_at: u32,
    },
    JumpIntoScope {
        label: Box<[u8]>,
        goto_at: u32,
        /// The declaration the jump would skip.
        variable: Box<[u8]>,
    },
}

/// What the `near` clause of a message shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Near {
    Eof,
    Buffer(Vec<u8>),
    None,
}

impl Error {
    pub fn line(&self, lines: &LineIndex) -> u32 {
        lines.line_of(self.at)
    }

    pub fn message(&self, lines: &LineIndex) -> Vec<u8> {
        let mut out = self.text(lines).into_bytes();

        match &self.near {
            Near::None => {}
            Near::Eof => out.extend_from_slice(b" near <eof>"),
            Near::Buffer(bytes) => {
                out.extend_from_slice(b" near '");
                out.extend_from_slice(bytes);
                out.push(b'\'');
            }
        }

        out
    }

    fn text(&self, lines: &LineIndex) -> String {
        match &self.kind {
            ErrorKind::MalformedNumber => "malformed number".to_owned(),
            ErrorKind::UnfinishedString => "unfinished string".to_owned(),
            ErrorKind::HexadecimalDigitExpected => "hexadecimal digit expected".to_owned(),
            ErrorKind::MissingOpenBrace => "missing '{'".to_owned(),
            ErrorKind::Utf8ValueTooLarge => "UTF-8 value too large".to_owned(),
            ErrorKind::MissingCloseBrace => "missing '}'".to_owned(),
            ErrorKind::DecimalEscapeTooLarge => "decimal escape too large".to_owned(),
            ErrorKind::InvalidEscapeSequence => "invalid escape sequence".to_owned(),
            ErrorKind::InvalidLongStringDelimiter => "invalid long string delimiter".to_owned(),
            ErrorKind::UnfinishedLongString { open_at } => format!(
                "unfinished long string (starting at line {})",
                lines.line_of(*open_at)
            ),
            ErrorKind::UnfinishedLongComment { open_at } => format!(
                "unfinished long comment (starting at line {})",
                lines.line_of(*open_at)
            ),
            ErrorKind::UnknownAttribute(name) => {
                format!("unknown attribute '{}'", String::from_utf8_lossy(name))
            }
            ErrorKind::MultipleToBeClosed => {
                "multiple to-be-closed variables in local list".to_owned()
            }
            ErrorKind::Expected(text) => format!("{text} expected"),
            ErrorKind::ExpectedToClose {
                expected,
                open,
                open_at,
            } => {
                let open_line = lines.line_of(*open_at);

                if open_line == lines.line_of(self.at) {
                    format!("{expected} expected")
                } else {
                    format!("{expected} expected (to close {open} at line {open_line})")
                }
            }
            ErrorKind::EqualsOrInExpected => "'=' or 'in' expected".to_owned(),
            ErrorKind::NameOrDotsExpected => "<name> or '...' expected".to_owned(),
            ErrorKind::FunctionArgumentsExpected => "function arguments expected".to_owned(),
            ErrorKind::UnexpectedSymbol => "unexpected symbol".to_owned(),
            ErrorKind::VarargsOutsideVarargFunction => {
                "cannot use '...' outside a vararg function".to_owned()
            }
            ErrorKind::SyntaxError => "syntax error".to_owned(),
            ErrorKind::BreakOutsideLoop => "break outside loop".to_owned(),
            ErrorKind::GlobalToBeClosed => "global variables cannot be to-be-closed".to_owned(),
            ErrorKind::ConstAssignment(name) => format!(
                "attempt to assign to const variable '{}'",
                String::from_utf8_lossy(name)
            ),
            ErrorKind::VariableNotDeclared(name) => {
                format!("variable '{}' not declared", String::from_utf8_lossy(name))
            }
            ErrorKind::EnvIsGlobal(name) => format!(
                "_ENV is global when accessing variable '{}'",
                String::from_utf8_lossy(name)
            ),
            ErrorKind::LabelAlreadyDefined { name, first_at } => format!(
                "label '{}' already defined on line {}",
                String::from_utf8_lossy(name),
                lines.line_of(*first_at)
            ),
            ErrorKind::NoVisibleLabel { name, goto_at } => format!(
                "no visible label '{}' for <goto> at line {}",
                String::from_utf8_lossy(name),
                lines.line_of(*goto_at)
            ),
            ErrorKind::JumpIntoScope {
                label,
                goto_at,
                variable,
            } => format!(
                "<goto {}> at line {} jumps into the scope of '{}'",
                String::from_utf8_lossy(label),
                lines.line_of(*goto_at),
                String::from_utf8_lossy(variable)
            ),
        }
    }
}
