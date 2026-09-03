//! One token per cell, and the branch that decides which kind it is.

use crate::error::Error;
use crate::token::{Token, TokenKind, keyword};

use super::Lexer;
use super::bytes::{is_name_part, is_name_start};

impl<'a> Lexer<'a> {
    /// The next token. At the end of the source this keeps returning `TokenKind::eof`.
    pub(crate) fn next_token(&mut self) -> Result<Token<'a>, Error> {
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
                        return Ok(self.token(TokenKind::Byte(b'-'), start));
                    }

                    self.pos += 1;
                    self.skip_comment()?;
                }

                b'[' => {
                    return match self.long_bracket_level() {
                        1 => Ok(self.token(TokenKind::Byte(b'['), start)),
                        0 => Err(self.delimiter_error(start)),
                        level => self.read_long_string(level, start),
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
                    } else if self.eat(b'>') {
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

                b'"' | b'\'' => return self.read_string(byte, start),

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
                        return self.read_numeral(start);
                    }

                    return Ok(self.token(TokenKind::Byte(b'.'), start));
                }

                b'0'..=b'9' => return self.read_numeral(start),

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
}
