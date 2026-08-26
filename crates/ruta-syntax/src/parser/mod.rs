//! The parser.

use crate::error::{Near, SyntaxError, SyntaxErrorKind};
use crate::lexer::Lexer;
use crate::token::{Token, TokenKind};

/// Reads one chunk. Nothing is accepted yet.
pub fn parse_chunk(source: &[u8]) -> Result<(), SyntaxError> {
    let mut parser = Parser::new(source)?;

    while !matches!(parser.current.kind, TokenKind::Eof) {
        parser.advance()?;
    }

    Err(SyntaxError {
        kind: SyntaxErrorKind::NotImplemented,
        at: parser.current.span.start,
        near: Near::None,
    })
}

#[derive(Debug)]
struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token<'a>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a [u8]) -> Result<Self, SyntaxError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;

        Ok(Self { lexer, current })
    }

    fn advance(&mut self) -> Result<(), SyntaxError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }
}
