//! The lexer.

mod bytes;
mod long;
mod number;
mod scan;
mod string;

use crate::error::{Near, SyntaxError, SyntaxErrorKind};
use crate::token::{Span, Token, TokenKind};

#[derive(Debug)]
pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    buf: Vec<u8>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            pos: 0,
            buf: Vec::new(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `\r\n` and `\n\r` are one break, `\n\n` is two.
    fn newline(&mut self) {
        let first = self.source[self.pos];
        self.pos += 1;

        if self
            .peek()
            .is_some_and(|byte| (byte == b'\n' || byte == b'\r') && byte != first)
        {
            self.pos += 1;
        }
    }

    fn token(&self, kind: TokenKind<'a>, start: usize) -> Token<'a> {
        Token {
            kind,
            span: Span::new(start as u32, self.pos as u32),
        }
    }

    fn save_and_next(&mut self) {
        self.buf.push(self.source[self.pos]);
        self.pos += 1;
    }

    fn eof_error(&self, kind: SyntaxErrorKind) -> SyntaxError {
        SyntaxError {
            kind,
            at: self.pos as u32,
            near: Near::Eof,
        }
    }

    fn buffered(&self, kind: SyntaxErrorKind) -> SyntaxError {
        SyntaxError {
            kind,
            at: self.pos as u32,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// The offending byte joins the buffer so that `near` shows it.
    fn escape_error(&mut self, kind: SyntaxErrorKind) -> SyntaxError {
        let at = self.pos as u32;
        if self.peek().is_some() {
            self.save_and_next();
        }

        SyntaxError {
            kind,
            at,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// `[=` with no second bracket.
    fn delimiter_error(&self, start: usize) -> SyntaxError {
        SyntaxError {
            kind: SyntaxErrorKind::InvalidLongStringDelimiter,
            at: self.pos as u32,
            near: Near::Buffer(self.source[start..self.pos].to_vec()),
        }
    }
}
