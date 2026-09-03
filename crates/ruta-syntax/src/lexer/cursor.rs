//! The cursor: the bytes the lexer is walking, and the primitives the scanners share.

use crate::error::{Error, ErrorKind, Near};
use crate::token::{Span, Token, TokenKind};

#[derive(Debug)]
pub(crate) struct Lexer<'a> {
    pub(super) source: &'a [u8],
    pub(super) pos: usize,
    pub(super) buf: Vec<u8>,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            pos: 0,
            buf: Vec::new(),
        }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    pub(super) fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `\r\n` and `\n\r` are one break, `\n\n` is two.
    pub(super) fn newline(&mut self) {
        let first = self.source[self.pos];
        self.pos += 1;

        if self
            .peek()
            .is_some_and(|byte| (byte == b'\n' || byte == b'\r') && byte != first)
        {
            self.pos += 1;
        }
    }

    pub(super) fn token(&self, kind: TokenKind<'a>, start: usize) -> Token<'a> {
        Token {
            kind,
            span: Span::new(start as u32, self.pos as u32),
        }
    }

    pub(super) fn save_and_next(&mut self) {
        self.buf.push(self.source[self.pos]);
        self.pos += 1;
    }

    pub(super) fn eof_error(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            at: self.pos as u32,
            near: Near::Eof,
        }
    }

    pub(super) fn buffered(&self, kind: ErrorKind) -> Error {
        Error {
            kind,
            at: self.pos as u32,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// The offending byte joins the buffer so that `near` shows it.
    pub(super) fn escape_error(&mut self, kind: ErrorKind) -> Error {
        let at = self.pos as u32;
        if self.peek().is_some() {
            self.save_and_next();
        }

        Error {
            kind,
            at,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// `[=` with no second bracket.
    pub(super) fn delimiter_error(&self, start: usize) -> Error {
        Error {
            kind: ErrorKind::InvalidLongStringDelimiter,
            at: self.pos as u32,
            near: Near::Buffer(self.source[start..self.pos].to_vec()),
        }
    }
}
