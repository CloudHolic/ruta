//! Short strings and their escapes.

use crate::token::{Token, TokenKind};

use super::Lexer;
use super::bytes::hex_value;
use super::error::{LexError, LexErrorKind};

impl<'a> Lexer<'a> {
    /// The buffer doubles as the string's value and as the text `near` shows on error, so an
    /// escape is decoded in place: `"\65\q"` reports `near '"A\q'`, note `near '"\65\q'`.
    pub(super) fn read_string(
        &mut self,
        delimiter: u8,
        start: usize,
    ) -> Result<Token<'a>, LexError> {
        self.buf.clear();
        self.save_and_next(); // keep the delimiter, for error messages

        loop {
            match self.peek() {
                Some(byte) if byte == delimiter => break,
                None => return Err(self.eof_error(LexErrorKind::UnfinishedString)),
                Some(b'\n' | b'\r') => return Err(self.buffered(LexErrorKind::UnfinishedString)),
                Some(b'\\') => self.read_escape()?,
                Some(_) => self.save_and_next(),
            }
        }

        self.save_and_next(); // skip the delimiter
        let value = self.buf[1..self.buf.len() - 1].to_vec();

        Ok(self.token(TokenKind::Str(value.into_boxed_slice()), start))
    }

    /// What on `\...` sequence stands for. The `\` is already in the buffer.
    fn read_escape(&mut self) -> Result<(), LexError> {
        self.save_and_next(); // keep the `\`, for error messages

        match self.peek() {
            // The string loop raises `unfinisehd string` on its next turn.
            None => {}

            Some(b'x') => {
                let byte = self.read_hex_escape()?;
                self.put_decoded(byte);
            }

            Some(b'u') => {
                let value = self.read_utf8_escape()?;
                utf8_encode(&mut self.buf, value);
            }

            // `\z` drops itself and the run of whitespace after it, line breaks included.
            Some(b'z') => {
                self.buf.pop();
                self.pos += 1;

                while let Some(byte) = self.peek() {
                    match byte {
                        b'\n' | b'\r' => self.newline(),
                        b' ' | b'\t' | 0x0b | 0x0c => self.pos += 1,
                        _ => break,
                    }
                }
            }

            // A `\` before a real line break stands for one '\n', however the break is spelled.
            Some(b'\n' | b'\r') => {
                self.newline();
                self.put_decoded(b'\n');
            }

            Some(byte) if byte.is_ascii_digit() => {
                let byte = self.read_decimal_escape()?;
                self.put_decoded(byte);
            }
            Some(byte) => match simple_escape(byte) {
                Some(decoded) => {
                    self.pos += 1;
                    self.put_decoded(decoded);
                }
                None => return Err(self.escape_error(LexErrorKind::InvalidEscapeSequence)),
            },
        }

        Ok(())
    }

    /// The `\` that opened the escape becomes the byte the escape stood for.
    fn put_decoded(&mut self, byte: u8) {
        self.buf.pop();
        self.buf.push(byte);
    }

    fn hex_digit(&mut self) -> Result<u32, LexError> {
        self.save_and_next();

        match self.peek() {
            Some(byte) if byte.is_ascii_hexdigit() => Ok(hex_value(byte)),
            _ => Err(self.escape_error(LexErrorKind::HexadecimalDigitExpected)),
        }
    }

    /// `\xNN`. Both digits sit in the buffer while they are read, so a bad one shows up in `near`.
    fn read_hex_escape(&mut self) -> Result<u8, LexError> {
        let value = (self.hex_digit()? << 4) + self.hex_digit()?;

        self.pos += 1;
        self.buf.truncate(self.buf.len() - 2);

        Ok(value as u8)
    }

    fn read_utf8_escape(&mut self) -> Result<u32, LexError> {
        let mut saved = 4; // '\', 'u', '{' and the first digit
        self.save_and_next(); // skip 'u'

        if self.peek() != Some(b'{') {
            return Err(self.escape_error(LexErrorKind::MissingOpenBrace));
        }

        let mut value = self.hex_digit()?;
        loop {
            self.save_and_next();
            let Some(digit) = self.peek().filter(|byte| byte.is_ascii_hexdigit()) else {
                break;
            };

            saved += 1;
            if value > 0x7fff_ffff >> 4 {
                return Err(self.escape_error(LexErrorKind::Utf8ValueTooLarge));
            }

            value = (value << 4) + hex_value(digit);
        }

        if self.peek() != Some(b'}') {
            return Err(self.escape_error(LexErrorKind::MissingCloseBrace));
        }

        self.pos += 1; // skip '}'
        self.buf.truncate(self.buf.len() - saved);

        Ok(value)
    }

    fn read_decimal_escape(&mut self) -> Result<u8, LexError> {
        let mut value = 0;
        let mut digits = 0;

        while digits < 3 {
            let Some(digit) = self.peek().filter(|byte| byte.is_ascii_digit()) else {
                break;
            };

            value = 10 * value + u32::from(digit - b'0');
            self.save_and_next();
            digits += 1;
        }

        if value > 255 {
            return Err(self.escape_error(LexErrorKind::DecimalEscapeTooLarge));
        }

        self.buf.truncate(self.buf.len() - digits);

        Ok(value as u8)
    }
}

/// The one-letter escapes.
fn simple_escape(byte: u8) -> Option<u8> {
    Some(match byte {
        b'a' => 0x07,
        b'b' => 0x08,
        b'f' => 0x0c,
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        b'v' => 0x0b,
        b'\\' | b'"' | b'\'' => byte,
        _ => return None,
    })
}

/// Lua's `\u` reaches 0x7FFFFFFF, past Unicode,
/// so this runs to six bytes rather than the four UTF-8 proper allows.
fn utf8_encode(out: &mut Vec<u8>, value: u32) {
    if value < 0x80 {
        out.push(value as u8);
        return;
    }

    let mut continuation = [0; 5];
    let mut count = 0;
    let mut rest = value;
    let mut first_fits = 0x3f;

    while {
        continuation[count] = 0x80 | (rest & 0x3f) as u8;
        count += 1;
        rest >>= 6;
        first_fits >>= 1;
        rest > first_fits
    } {}

    out.push(((!first_fits << 1) | rest) as u8);
    for byte in continuation[..count].iter().rev() {
        out.push(*byte);
    }
}
