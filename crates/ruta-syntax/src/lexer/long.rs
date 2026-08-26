//! Long brackets: `[[...]]` strings and `--[[...]]` comments.

use crate::error::{SyntaxError, SyntaxErrorKind};
use crate::token::{Token, TokenKind};

use super::Lexer;

/// Which long-bracket form is being read.
#[derive(Debug, Clone, Copy)]
enum LongForm {
    String,
    Comment,
}

impl<'a> Lexer<'a> {
    /// A long string. The opening delimiter and its level are already behind.
    pub(super) fn read_long_string(
        &mut self,
        level: usize,
        start: usize,
    ) -> Result<Token<'a>, SyntaxError> {
        self.read_long_bracket(level, start, LongForm::String)?;
        let value: Box<[u8]> = self.buf.as_slice().into();

        Ok(self.token(TokenKind::Str(value), start))
    }

    /// Consumes a `[` or `]` and the run of `=` after it, and reports the level:
    /// `count + 2` for a complete delimiter, 1 for a lone bracket, 0 for `[=` with no second bracket.
    pub(super) fn long_bracket_level(&mut self) -> usize {
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

    /// A comment, long or short. The `--` is already consumed.
    pub(super) fn skip_comment(&mut self) -> Result<(), SyntaxError> {
        if self.peek() == Some(b'[') {
            let open_at = self.pos;
            if let level @ 2.. = self.long_bracket_level() {
                return self.read_long_bracket(level, open_at, LongForm::Comment);
            }

            // A lone '[' or '[=' with no second bracket: an ordinary comment after all,
            // and the bytes just consumed belong to it.
        }

        while let Some(byte) = self.peek() {
            if byte == b'\n' || byte == b'\r' {
                break;
            }

            self.pos += 1;
        }

        Ok(())
    }

    /// The body between `[[` and `]]`, at whatever level. Fills the buffer with the content.
    fn read_long_bracket(
        &mut self,
        level: usize,
        open_at: usize,
        form: LongForm,
    ) -> Result<(), SyntaxError> {
        self.buf.clear();
        self.pos += 1; // the second bracket

        if self
            .peek()
            .is_some_and(|byte| byte == b'\n' || byte == b'\r')
        {
            self.newline();
        }

        loop {
            match self.peek() {
                None => {
                    let open_at = open_at as u32;

                    return Err(self.eof_error(match form {
                        LongForm::String => SyntaxErrorKind::UnfinishedLongString { open_at },
                        LongForm::Comment => SyntaxErrorKind::UnfinishedLongComment { open_at },
                    }));
                }

                // A closing run that does not match this level is content,
                // and the scan has already stepped over it.
                Some(b']') => {
                    let at = self.pos;
                    if self.long_bracket_level() == level {
                        self.pos += 1;
                        return Ok(());
                    }

                    let source = self.source;
                    self.buf.extend_from_slice(&source[at..self.pos]);
                }

                Some(b'\n' | b'\r') => {
                    self.buf.push(b'\n');
                    self.newline();
                }

                Some(byte) => {
                    self.buf.push(byte);
                    self.pos += 1;
                }
            }
        }
    }
}
