//! What the compiler refuses, and how it says so.

use crate::line_index::LineIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub kind: SyntaxErrorKind,
    /// Byte offset the error is reported at.
    pub at: u32,
    pub near: Near,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxErrorKind {
    MalformedNumber,
    UnfinishedString,
    HexadecimalDigitExpected,
    MissingOpenBrace,
    Utf8ValueTooLarge,
    MissingCloseBrace,
    DecimalEscapeTooLarge,
    InvalidEscapeSequence,
    InvalidLongStringDelimiter,
    UnfinishedLongString { open_at: u32 },
    UnfinishedLongComment { open_at: u32 },

    NotImplemented,
}

/// What the `near` clause of a message shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Near {
    Eof,
    Buffer(Vec<u8>),
    None,
}

impl SyntaxError {
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
        match self.kind {
            SyntaxErrorKind::MalformedNumber => "malformed number".to_owned(),
            SyntaxErrorKind::UnfinishedString => "unfinished string".to_owned(),
            SyntaxErrorKind::HexadecimalDigitExpected => "hexadecimal digit expected".to_owned(),
            SyntaxErrorKind::MissingOpenBrace => "missing '{'".to_owned(),
            SyntaxErrorKind::Utf8ValueTooLarge => "UTF-8 value too large".to_owned(),
            SyntaxErrorKind::MissingCloseBrace => "missing '}'".to_owned(),
            SyntaxErrorKind::DecimalEscapeTooLarge => "decimal escape too large".to_owned(),
            SyntaxErrorKind::InvalidEscapeSequence => "invalid escape sequence".to_owned(),
            SyntaxErrorKind::InvalidLongStringDelimiter => {
                "invalid long string delimiter".to_owned()
            }
            SyntaxErrorKind::UnfinishedLongString { open_at } => format!(
                "unfinished long string (starting at line {})",
                lines.line_of(open_at)
            ),
            SyntaxErrorKind::UnfinishedLongComment { open_at } => format!(
                "unfinished long comment (starting at line {})",
                lines.line_of(open_at)
            ),
            SyntaxErrorKind::NotImplemented => "parsing is not implemented".to_owned(),
        }
    }
}
