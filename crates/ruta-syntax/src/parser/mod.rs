//! The parser.

mod expr;
mod func;
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
    source: &'a [u8],
    lexer: Lexer<'a>,
    builder: Builder<'a>,
    current: Token<'a>,
    /// One token past `current`, once something has asked for it.
    ahead: Option<Token<'a>>,
    /// Where the last consumed token ended.
    last_end: u32,
    /// Whether the function being parsed takes `...`.
    varargs: bool,
    /// Loops open in the function being parsed.
    loops: u32,
}

impl<'a> Parser<'a> {
    fn new(source: &'a [u8]) -> Result<Self, SyntaxError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;

        Ok(Self {
            source,
            lexer,
            builder: Builder::default(),
            current,
            ahead: None,
            last_end: 0,
            varargs: true,
            loops: 0,
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

    /// Consumes a keyword or a multi-byte symbol when that is what is next.
    fn eat(&mut self, kind: TokenKind<'static>) -> Result<bool, SyntaxError> {
        if self.current.kind != kind {
            return Ok(false);
        }

        self.advance()?;
        Ok(true)
    }

    /// The same, for a token the grammar leaves no choice about.
    fn expect(&mut self, kind: TokenKind<'static>) -> Result<(), SyntaxError> {
        if self.current.kind == kind {
            return self.advance();
        }

        Err(self.syntax(SyntaxErrorKind::Expected(kind.describe().into())))
    }

    /// The same, for a token that closes one opened earlier.
    fn expect_match(
        &mut self,
        kind: TokenKind<'static>,
        open: TokenKind<'static>,
        open_at: u32,
    ) -> Result<(), SyntaxError> {
        if self.current.kind == kind {
            return self.advance();
        }

        Err(self.syntax(SyntaxErrorKind::ExpectedToClose {
            expected: kind.describe().into(),
            open: open.describe().into(),
            open_at,
        }))
    }

    fn name(&mut self) -> Result<&'a [u8], SyntaxError> {
        let Some(name) = self.current_name() else {
            let expected = TokenKind::Name(b"").describe();
            return Err(self.syntax(SyntaxErrorKind::Expected(expected.into())));
        };
        self.advance()?;

        Ok(name)
    }

    fn current_name(&self) -> Option<&'a [u8]> {
        match self.current.kind {
            TokenKind::Name(name) => Some(name),
            _ => None,
        }
    }

    /// From `start` to the end of the last consumed token.
    fn span_from(&self, start: u32) -> Span {
        Span::new(start, self.last_end.max(start))
    }

    /// A syntax error.
    fn syntax(&self, kind: SyntaxErrorKind) -> SyntaxError {
        SyntaxError {
            kind,
            at: self.current.span.end,
            near: self.near(),
        }
    }

    /// A semantic error: no `near` clause, and the line is the one hte last consumed token
    /// ended on rather than the line the parser has reached.
    fn semantic(&self, kind: SyntaxErrorKind) -> SyntaxError {
        SyntaxError {
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
