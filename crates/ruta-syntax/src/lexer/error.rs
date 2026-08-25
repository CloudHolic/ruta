//! What the lexer refuses, and how it says so.

use crate::line_index::LineIndex;

use super::Lexer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    /// Byte offset the error is reported at.
    pub at: u32,
    pub near: Near,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
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
}

/// What the `near` clause of a message shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Near {
    Eof,
    Buffer(Vec<u8>),
    None,
}

impl LexError {
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
            LexErrorKind::MalformedNumber => "malformed number".to_owned(),
            LexErrorKind::UnfinishedString => "unfinished string".to_owned(),
            LexErrorKind::HexadecimalDigitExpected => "hexadecimal digit expected".to_owned(),
            LexErrorKind::MissingOpenBrace => "missing '{'".to_owned(),
            LexErrorKind::Utf8ValueTooLarge => "UTF-8 value too large".to_owned(),
            LexErrorKind::MissingCloseBrace => "missing '}'".to_owned(),
            LexErrorKind::DecimalEscapeTooLarge => "decimal escape too large".to_owned(),
            LexErrorKind::InvalidEscapeSequence => "invalid escape sequence".to_owned(),
            LexErrorKind::InvalidLongStringDelimiter => "invalid long string delimiter".to_owned(),
            LexErrorKind::UnfinishedLongString { open_at } => format!(
                "unfinished long string (starting at line {})",
                lines.line_of(open_at)
            ),
            LexErrorKind::UnfinishedLongComment { open_at } => format!(
                "unfinished long comment (starting at line {})",
                lines.line_of(open_at)
            ),
        }
    }
}

impl<'a> Lexer<'a> {
    pub(super) fn eof_error(&self, kind: LexErrorKind) -> LexError {
        LexError {
            kind,
            at: self.pos as u32,
            near: Near::Eof,
        }
    }

    pub(super) fn buffered(&self, kind: LexErrorKind) -> LexError {
        LexError {
            kind,
            at: self.pos as u32,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// The offending byte joins the buffer so that `near` shows it.
    pub(super) fn escape_error(&mut self, kind: LexErrorKind) -> LexError {
        let at = self.pos as u32;
        if self.peek().is_some() {
            self.save_and_next();
        }

        LexError {
            kind,
            at,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// `[=` with no second bracket.
    pub(super) fn delimiter_error(&self, start: usize) -> LexError {
        LexError {
            kind: LexErrorKind::InvalidLongStringDelimiter,
            at: self.pos as u32,
            near: Near::Buffer(self.source[start..self.pos].to_vec()),
        }
    }
}
