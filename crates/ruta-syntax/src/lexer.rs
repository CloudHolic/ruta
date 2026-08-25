//! The lexer. The structure follows `llex.c`.

use crate::token::{Span, Token, TokenKind, keyword};

#[derive(Debug)]
pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    /// Byte offset the error is reported at.
    pub at: u32,
    pub near: Near,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexErrorKind {
    NotImplemented,
}

/// What the `near` clause of a message shows - `txtToken`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Near {
    Eof,
    /// The partial token read so far, sas bytes of the source.
    Buffer(Span),
    None,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self { source, pos: 0 }
    }

    /// The next token. At the end of the source this keeps returning `TokenKind::eof`.
    pub fn next_token(&mut self) -> Result<Token<'a>, LexError> {
        loop {
            let start = self.pos;
            let Some(byte) = self.peek() else {
                return Ok(self.token(TokenKind::Eof, start));
            };

            match byte {
                b'\n' | b'\r' => self.newline(),
                b' ' | b'\t' | 0x0b | 0x0c => self.pos += 1,

                b'-' => {
                    self.pos += 1;
                    if self.peek() != Some(b'-') {
                        return Ok(self.token(TokenKind::Eof, start));
                    }

                    self.pos += 1;
                    if self.peek() == Some(b'[') && self.long_bracket_level() >= 2 {
                        return Err(self.not_implemented(start));
                    }

                    // A short comment runs to the end of the line either way, so the bytes
                    // `long_bracket_level` consumed above do not need giving back.
                    while let Some(byte) = self.peek() {
                        if byte == b'\n' || byte == b'\r' {
                            break;
                        }
                        self.pos += 1;
                    }
                }

                b'[' => {
                    return match self.long_bracket_level() {
                        1 => Ok(self.token(TokenKind::Byte(b'['), start)),
                        _ => Err(self.not_implemented(start)),
                    };
                }

                b'=' => {
                    self.pos += 1;
                    return Ok(if self.eat(b'=') {
                        self.token(TokenKind::Eq, start)
                    } else {
                        self.token(TokenKind::Byte(b'='), start)
                    });
                }

                b'<' => {
                    self.pos += 1;
                    return Ok(if self.eat(b'=') {
                        self.token(TokenKind::Le, start)
                    } else if self.eat(b'<') {
                        self.token(TokenKind::Shl, start)
                    } else {
                        self.token(TokenKind::Byte(b'<'), start)
                    });
                }

                b'>' => {
                    self.pos += 1;
                    return Ok(if self.eat(b'=') {
                        self.token(TokenKind::Ge, start)
                    } else if self.eat(b'<') {
                        self.token(TokenKind::Shr, start)
                    } else {
                        self.token(TokenKind::Byte(b'>'), start)
                    });
                }

                b'/' => {
                    self.pos += 1;
                    return Ok(if self.eat(b'/') {
                        self.token(TokenKind::IDiv, start)
                    } else {
                        self.token(TokenKind::Byte(b'/'), start)
                    });
                }

                b'~' => {
                    self.pos += 1;
                    return Ok(if self.eat(b'=') {
                        self.token(TokenKind::Ne, start)
                    } else {
                        self.token(TokenKind::Byte(b'~'), start)
                    });
                }

                b':' => {
                    self.pos += 1;
                    return Ok(if self.eat(b':') {
                        self.token(TokenKind::DbColon, start)
                    } else {
                        self.token(TokenKind::Byte(b':'), start)
                    });
                }

                b'"' | b'\'' => return Err(self.not_implemented(start)),

                b'.' => {
                    self.pos += 1;
                    if self.eat(b'.') {
                        return Ok(if self.eat(b'.') {
                            self.token(TokenKind::Dots, start)
                        } else {
                            self.token(TokenKind::Concat, start)
                        });
                    }

                    if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                        return Err(self.not_implemented(start));
                    }

                    return Ok(self.token(TokenKind::Byte(b'.'), start));
                }

                b'0'..=b'9' => return Err(self.not_implemented(start)),

                byte if is_name_start(byte) => {
                    while self.peek().is_some_and(is_name_part) {
                        self.pos += 1;
                    }

                    let word = &self.source[start..self.pos];
                    let kind = keyword(word).unwrap_or(TokenKind::Name(word));

                    return Ok(self.token(kind, start));
                }

                byte => {
                    self.pos += 1;
                    return Ok(self.token(TokenKind::Byte(byte), start));
                }
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    /// `check_next1`
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `inclinenumber`: `\r\n` and `\n\r` are one break, `\n\n` is two.
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

    /// `skip_sep`: Consumes the bracket and its run of `=`, and reports `count + 2` for a complete opener, 1 for a lone `[`, 0 for `[=` with no second `[`.
    fn long_bracket_level(&mut self) -> usize {
        let opener = self.source[self.pos];
        self.pos += 1;

        let mut count = 0;
        while self.eat(b'=') {
            count += 1;
        }

        if self.peek() == Some(opener) {
            count + 2
        } else if count == 0 {
            1
        } else {
            0
        }
    }

    fn token(&self, kind: TokenKind<'a>, start: usize) -> Token<'a> {
        Token {
            kind,
            span: Span::new(start as u32, self.pos as u32),
        }
    }

    fn not_implemented(&self, start: usize) -> LexError {
        LexError {
            kind: LexErrorKind::NotImplemented,
            at: start as u32,
            near: Near::None,
        }
    }
}

impl LexError {
    pub fn line(&self, lines: &LineIndex) -> u32 {
        lines.line_of(self.at)
    }

    /// The message and its `near` clause, assembled as `lexerror` does.
    /// Bytes rather than a `String`: the partial token can hold anything, and the comparison against the reference is byte for byte.
    pub fn message(&self, source: &[u8]) -> Vec<u8> {
        let text: &str = match self.kind {
            LexErrorKind::NotImplemented => "lexing is not implemented here",
        };

        let mut out = text.as_bytes().to_vec();
        match self.near {
            Near::None => {}
            Near::Eof => out.extend_from_slice(b" near <eof>"),
            Near::Buffer(span) => {
                out.extend_from_slice(b" near '");
                out.extend_from_slice(&source[span.start as usize..span.end as usize]);
                out.push(b'\'');
            }
        }

        out
    }
}

///Where each line starts, so that a byte offset can be turned into a line number without every token carrying one.
#[derive(Debug)]
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(source: &[u8]) -> Self {
        let mut starts = vec![0];
        let mut at = 0;

        while at < source.len() {
            let byte = source[at];
            at += 1;
            if byte == b'\n' || byte == b'\r' {
                if source
                    .get(at)
                    .is_some_and(|next| (*next == b'\n' || *next == b'\r') && *next != byte)
                {
                    at += 1;
                }

                starts.push(at as u32);
            }
        }

        Self { starts }
    }

    /// The 1-based line holding `offset`.
    pub fn line_of(&self, offset: u32) -> u32 {
        match self.starts.binary_search(&offset) {
            Ok(index) => index as u32 + 1,
            Err(index) => index as u32,
        }
    }
}

/// `lislalpha`: ASCII letters and `_`, and nothing else - a byte above 127 is a token of its own, not part of a name.
fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

/// `lislalnum`.
fn is_name_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
