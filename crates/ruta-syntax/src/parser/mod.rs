//! The parser.

mod expr;
mod stat;

use crate::ast::{Ast, Builder};
use crate::error::{Near, SyntaxError, SyntaxErrorKind};
use crate::lexer::Lexer;
use crate::token::{Span, Token, TokenKind};

/// Reads one chunk.
pub fn parse_chunk(source: &[u8]) -> Result<Ast<'_>, SyntaxError> {
    let mut parser = Parser::new(source)?;
    let main = parser.chunk()?;

    Ok(parser.builder.finish(main))
}

#[derive(Debug)]
struct Parser<'a> {
    lexer: Lexer<'a>,
    builder: Builder<'a>,
    current: Token<'a>,
    /// One token past `current`, once something has asked for it.
    ahead: Option<Token<'a>>,
    /// Where the last consumed token ended.
    last_end: u32,
}

impl<'a> Parser<'a> {
    fn new(source: &'a [u8]) -> Result<Self, SyntaxError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;

        Ok(Self {
            lexer,
            builder: Builder::default(),
            current,
            ahead: None,
            last_end: 0,
        })
    }

    fn advance(&mut self) -> Result<(), SyntaxError> {
        self.last_end = self.current.span.end;
        self.current = match self.ahead.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };

        Ok(())
    }

    /// The token after `current`.
    fn peek(&mut self) -> Result<&Token<'a>, SyntaxError> {
        if self.ahead.is_none() {
            self.ahead = Some(self.lexer.next_token()?);
        }

        Ok(self.ahead.as_ref().expect("just filled"))
    }

    fn at_byte(&self, byte: u8) -> bool {
        self.current.kind == TokenKind::Byte(byte)
    }

    fn eat_byte(&mut self, byte: u8) -> Result<bool, SyntaxError> {
        if !self.at_byte(byte) {
            return Ok(false);
        }

        self.advance()?;
        Ok(true)
    }

    fn current_name(&self) -> Option<&'a [u8]> {
        match self.current.kind {
            TokenKind::Name(name) => Some(name),
            _ => None,
        }
    }

    /// From `start` to the end of the last consumed token.
    fn span_from(&self, start: u32) -> Span {
        Span::new(start, self.last_end)
    }

    /// What the grammar parser cannot handle yet.
    fn not_implemented(&mut self) -> SyntaxError {
        let at = self.current.span.start;

        loop {
            match self.lexer.next_token() {
                Ok(token) if matches!(token.kind, TokenKind::Eof) => break,
                Ok(_) => {}
                Err(error) => return error,
            }
        }

        SyntaxError {
            kind: SyntaxErrorKind::NotImplemented,
            at,
            near: Near::None,
        }
    }
}
