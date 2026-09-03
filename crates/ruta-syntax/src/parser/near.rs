//! How a message names the token it is about.

use crate::error::{Error, ErrorKind, Near};
use crate::token::TokenKind;

use super::chunk::Parser;

impl<'a> Parser<'a> {
    /// A syntax error.
    pub(super) fn syntax(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            at: self.current.span.end,
            near: self.near(),
        }
    }

    /// A semantic error: no `near` clause, and the line is the one hte last consumed token
    /// ended on rather than the line the parser has reached.
    pub(super) fn semantic(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            at: self.last_end,
            near: Near::None,
        }
    }

    fn near(&self) -> Near {
        match &self.current.kind {
            TokenKind::Eof => Near::Eof,
            // A NUL byte token carries the same value as the marker for "no token at all".
            TokenKind::Byte(0) => Near::None,
            TokenKind::Str(value) => Near::Buffer(self.string_near(value)),
            TokenKind::Name(_) | TokenKind::Int(_) | TokenKind::Float(_) => {
                Near::Buffer(self.token_text().to_vec())
            }
            written => Near::Buffer(written.symbol()),
        }
    }

    /// The source text the current token was written as.
    fn token_text(&self) -> &'a [u8] {
        &self.source[self.current.span.start as usize..self.current.span.end as usize]
    }

    /// A short string shows its delimiters around the value, so the escapes it was written
    /// with are already resolved. A long bracket shows its own text instead, minus the
    /// newline that follows the opening bracket and never belonged to the value.
    fn string_near(&self, value: &[u8]) -> Vec<u8> {
        let text = self.token_text();
        let delimiter = text[0];

        if delimiter == b'"' || delimiter == b'\'' {
            let mut out = Vec::with_capacity(value.len() + 2);

            out.push(delimiter);
            out.extend_from_slice(value);
            out.push(delimiter);

            return out;
        }

        let open = text
            .iter()
            .skip(1)
            .take_while(|byte| **byte == b'=')
            .count()
            + 2;
        let mut out = text[..open].to_vec();
        out.extend_from_slice(&text[open + newline_len(&text[open..])..]);

        out
    }
}

/// How many bytes the line break at the front of `bytes` takes, if there is one.
fn newline_len(bytes: &[u8]) -> usize {
    match bytes {
        [b'\n', b'\r', ..] | [b'\r', b'\n', ..] => 2,
        [b'\n' | b'\r', ..] => 1,
        _ => 0,
    }
}
