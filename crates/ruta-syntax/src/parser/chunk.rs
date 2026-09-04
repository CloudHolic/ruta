//! The parser's state: the token window it reads through, and the depth it is allowed to recurse to.

use crate::ast::{Ast, Builder, Var};
use crate::error::{Error, ErrorKind};
use crate::lexer::Lexer;
use crate::token::{Span, Token, TokenKind};

/// How many nested `subexpr` and `block` entries the parser takes before it refuses the chunk.
const MAX_DEPTH: u32 = 1000;

#[derive(Debug)]
pub(super) struct Parser<'a> {
    pub(super) source: &'a [u8],
    pub(super) builder: Builder<'a>,
    pub(super) current: Token<'a>,
    /// Where the last consumed token ended.
    pub(super) last_end: u32,
    /// Whether the function being parsed takes `...`.
    pub(super) varargs: bool,
    /// Loops open in the function being parsed.
    pub(super) loops: u32,

    lexer: Lexer<'a>,
    /// One token past `current`, once something has asked for it.
    ahead: Option<Token<'a>>,
    /// Nested `subexpr` and `block` entries.
    depth: u32,
}

impl<'a> Parser<'a> {
    pub(super) fn advance(&mut self) -> Result<(), Error> {
        self.last_end = self.current.span.end;
        self.current = match self.ahead.take() {
            Some(token) => token,
            None => self.lexer.next_token()?,
        };

        Ok(())
    }

    /// The token after `current`.
    pub(super) fn peek(&mut self) -> Result<&Token<'a>, Error> {
        if self.ahead.is_none() {
            self.ahead = Some(self.lexer.next_token()?);
        }

        Ok(self.ahead.as_ref().expect("just filled"))
    }

    pub(super) fn at_byte(&self, byte: u8) -> bool {
        self.current.kind == TokenKind::Byte(byte)
    }

    pub(super) fn eat_byte(&mut self, byte: u8) -> Result<bool, Error> {
        if !self.at_byte(byte) {
            return Ok(false);
        }

        self.advance()?;
        Ok(true)
    }

    /// Consumes a keyword or a multi-byte symbol when that is what is next.
    pub(super) fn eat(&mut self, kind: TokenKind<'static>) -> Result<bool, Error> {
        if self.current.kind != kind {
            return Ok(false);
        }

        self.advance()?;
        Ok(true)
    }

    /// The same, for a token the grammar leaves no choice about.
    pub(super) fn expect(&mut self, kind: TokenKind<'static>) -> Result<(), Error> {
        if self.current.kind == kind {
            return self.advance();
        }

        Err(self.syntax(ErrorKind::Expected(kind.describe().into())))
    }

    /// The same, for a token that closes one opened earlier.
    pub(super) fn expect_match(
        &mut self,
        kind: TokenKind<'static>,
        open: TokenKind<'static>,
        open_at: u32,
    ) -> Result<(), Error> {
        if self.current.kind == kind {
            return self.advance();
        }

        Err(self.syntax(ErrorKind::ExpectedToClose {
            expected: kind.describe().into(),
            open: open.describe().into(),
            open_at,
        }))
    }

    pub(super) fn name(&mut self) -> Result<&'a [u8], Error> {
        let Some(name) = self.current_name() else {
            let expected = TokenKind::Name(b"").describe();
            return Err(self.syntax(ErrorKind::Expected(expected.into())));
        };
        self.advance()?;

        Ok(name)
    }

    /// A name being declared, numbered so that every use of it can point back here.
    pub(super) fn var(&mut self) -> Result<Var<'a>, Error> {
        let name = self.name()?;

        Ok(Var {
            name,
            id: self.builder.var(),
        })
    }

    pub(super) fn current_name(&self) -> Option<&'a [u8]> {
        match self.current.kind {
            TokenKind::Name(name) => Some(name),
            _ => None,
        }
    }

    /// From `start` to the end of the last consumed token.
    pub(super) fn span_from(&self, start: u32) -> Span {
        Span::new(start, self.last_end.max(start))
    }

    /// Takes one level of recursion.
    pub(super) fn descend(&mut self) -> Result<(), Error> {
        self.depth += 1;

        if self.depth > MAX_DEPTH {
            return Err(self.semantic(ErrorKind::StackOverflow));
        }

        Ok(())
    }

    /// Gives back a level taken by `descend`.
    pub(super) fn ascend(&mut self) {
        self.depth -= 1;
    }

    fn new(source: &'a [u8]) -> Result<Self, Error> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;

        Ok(Self {
            source,
            builder: Builder::default(),
            current,
            last_end: 0,
            varargs: true,
            loops: 0,

            lexer,
            ahead: None,
            depth: 0,
        })
    }
}

/// Reads one chunk.
pub fn parse_chunk(source: &[u8]) -> Result<Ast<'_>, Error> {
    let mut parser = Parser::new(source)?;
    let main = parser.chunk()?;

    Ok(parser.builder.finish(main))
}
