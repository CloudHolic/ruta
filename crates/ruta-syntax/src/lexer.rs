//! The lexer.

use std::f64;

use crate::token::{Span, Token, TokenKind, keyword};

#[derive(Debug)]
pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    /// `LexState.buff`
    buf: Vec<u8>,
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
    MalformedNumber,
    UnfinishedString,
    HexadecimalDigitExpected,
    MissingOpenBrace,
    Utf8ValueTooLarge,
    MissingCloseBrace,
    DecimalEscapeTooLarge,
    InvalidEscapeSequence,
    InvalidLongStringDelimiter,
    UnfinishedLongString { open_at: u32 },
    UnfinishedLongComment { open_at: u32 },
}

/// What the `near` clause of a message shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Near {
    Eof,
    /// The partial token read so far, sas bytes of the source.
    /// `read_string` decodes escapes in place, so `"\65\q"` reports `"A\q"`.
    Buffer(Vec<u8>),
    None,
}

/// Which long-bracket form is being read.
#[derive(Debug, Clone, Copy)]
enum LongForm {
    String,
    Comment,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            pos: 0,
            buf: Vec::new(),
        }
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
                        return Ok(self.token(TokenKind::Byte(b'-'), start));
                    }

                    self.pos += 1;
                    self.skip_comment()?;
                }

                b'[' => {
                    return match self.long_bracket_level() {
                        1 => Ok(self.token(TokenKind::Byte(b'['), start)),
                        0 => Err(self.delimiter_error(start)),
                        level => {
                            self.read_long_bracket(level, start, LongForm::String)?;
                            let value: Box<[u8]> = self.buf.as_slice().into();

                            Ok(self.token(TokenKind::Str(value), start))
                        }
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

    /// Consumes a `[` or `]` and the run of `=` after it, and reports the level:
    /// `count + 2` for a complete delimiter, 1 for a lone bracket, 0 for `[=` with no second bracket.
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

    /// A comment, long or short. The `--` is already consumed.
    fn skip_comment(&mut self) -> Result<(), LexError> {
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
    ) -> Result<(), LexError> {
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
                        LongForm::String => LexErrorKind::UnfinishedLongString { open_at },
                        LongForm::Comment => LexErrorKind::UnfinishedLongComment { open_at },
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

    /// A numeral. The scan is greedy and the conversion is the judge.
    fn read_numeral(&mut self, start: usize) -> Result<Token<'a>, LexError> {
        let hexadecimal =
            self.peek() == Some(b'0') && matches!(self.source.get(self.pos + 1), Some(b'x' | b'X'));

        self.pos += if hexadecimal { 2 } else { 1 };
        let exponent: [u8; 2] = if hexadecimal {
            [b'p', b'P']
        } else {
            [b'e', b'E']
        };

        loop {
            match self.peek() {
                Some(byte) if exponent.contains(&byte) => {
                    self.pos += 1;
                    if self.peek().is_some_and(|byte| byte == b'+' || byte == b'-') {
                        self.pos += 1;
                    }
                }
                Some(byte) if byte.is_ascii_hexdigit() || byte == b'.' => self.pos += 1,
                _ => break,
            }
        }

        if self.peek().is_some_and(is_name_start) {
            self.pos += 1;
        }

        let text = &self.source[start..self.pos];
        match numeral_value(text) {
            Some(kind) => Ok(self.token(kind, start)),
            None => Err(LexError {
                kind: LexErrorKind::MalformedNumber,
                at: start as u32,
                near: Near::Buffer(text.to_vec()),
            }),
        }
    }

    /// `[=` with no second bracket.
    fn delimiter_error(&self, start: usize) -> LexError {
        LexError {
            kind: LexErrorKind::InvalidLongStringDelimiter,
            at: self.pos as u32,
            near: Near::Buffer(self.source[start..self.pos].to_vec()),
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

    /// The `\` that opened the escape becomes the byte the escape stood for.
    fn put_decoded(&mut self, byte: u8) {
        self.buf.pop();
        self.buf.push(byte);
    }

    fn eof_error(&self, kind: LexErrorKind) -> LexError {
        LexError {
            kind,
            at: self.pos as u32,
            near: Near::Eof,
        }
    }

    fn buffered(&self, kind: LexErrorKind) -> LexError {
        LexError {
            kind,
            at: self.pos as u32,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// The offending byte joins the buffer so that `near` shows it.
    fn escape_error(&mut self, kind: LexErrorKind) -> LexError {
        let at = self.pos as u32;
        if self.peek().is_some() {
            self.save_and_next();
        }

        LexError {
            kind,
            at,
            near: Near::Buffer(self.buf.clone()),
        }
    }

    /// The buffer doubles as the string's value and as the text `near` shows on error, so an
    /// escape is decoded in place: `"\65\q"` reports `near '"A\q'`, note `near '"\65\q'`.
    fn read_string(&mut self, delimiter: u8, start: usize) -> Result<Token<'a>, LexError> {
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
            Some(byte) => match simple_esacpe(byte) {
                Some(decoded) => {
                    self.pos += 1;
                    self.put_decoded(decoded);
                }
                None => return Err(self.escape_error(LexErrorKind::InvalidEscapeSequence)),
            },
        }

        Ok(())
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

impl LexError {
    pub fn line(&self, lines: &LineIndex) -> u32 {
        lines.line_of(self.at)
    }

    pub fn message(&self, lines: &LineIndex) -> Vec<u8> {
        let mut out = self.text(lines).into_bytes();

        match &self.near {
            Near::None => {}
            Near::Eof => out.extend_from_slice(b" near <eof>"),
            Near::Buffer(bytes) => {
                out.extend_from_slice(b" near '");
                out.extend_from_slice(bytes);
                out.push(b'\'');
            }
        }

        out
    }

    fn text(&self, lines: &LineIndex) -> String {
        match self.kind {
            LexErrorKind::MalformedNumber => "malformed number".to_owned(),
            LexErrorKind::UnfinishedString => "unfinished string".to_owned(),
            LexErrorKind::HexadecimalDigitExpected => "hexadecimal digit expected".to_owned(),
            LexErrorKind::MissingOpenBrace => "missing '{'".to_owned(),
            LexErrorKind::Utf8ValueTooLarge => "UTF-8 value too large".to_owned(),
            LexErrorKind::MissingCloseBrace => "missing '}'".to_owned(),
            LexErrorKind::DecimalEscapeTooLarge => "decimal escape too large".to_owned(),
            LexErrorKind::InvalidEscapeSequence => "invalid escape sequence".to_owned(),
            LexErrorKind::InvalidLongStringDelimiter => "invalid long string delimiter".to_owned(),
            LexErrorKind::UnfinishedLongString { open_at } => format!(
                "unfinished long string (starting at line {})",
                lines.line_of(open_at)
            ),
            LexErrorKind::UnfinishedLongComment { open_at } => format!(
                "unfinished long comment (starting at line {})",
                lines.line_of(open_at)
            ),
        }
    }
}

/// Where each line starts, so that a byte offset can be turned into a line number without every token carrying one.
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

/// The one-letter escapes.
fn simple_esacpe(byte: u8) -> Option<u8> {
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

/// The byte must already be a hex digit.
fn hex_value(byte: u8) -> u32 {
    if byte.is_ascii_digit() {
        u32::from(byte - b'0')
    } else {
        u32::from(byte.to_ascii_lowercase() - b'a') + 10
    }
}

/// The value a numeral stands for, or `None` when the bytes are not one after all.
fn numeral_value(text: &[u8]) -> Option<TokenKind<'static>> {
    match text {
        [b'0', b'x' | b'X', rest @ ..] => hexadecimal_value(rest),
        _ => decimal_value(text),
    }
}

/// An integer unless a `.` or an exponent says otherwise,
/// and a float when the digits will not fit an `i64`.
fn decimal_value(text: &[u8]) -> Option<TokenKind<'static>> {
    let text = core::str::from_utf8(text).ok()?;

    if text.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(value) = text.parse::<i64>()
    {
        return Some(TokenKind::Int(value));
    }

    text.parse::<f64>().ok().map(TokenKind::Float)
}

/// Everything after the `0x`.
fn hexadecimal_value(text: &[u8]) -> Option<TokenKind<'static>> {
    if text.is_empty() {
        return None;
    }

    if text.iter().all(u8::is_ascii_hexdigit) {
        let mut value: u64 = 0;
        for byte in text {
            value = value
                .wrapping_mul(16)
                .wrapping_add(u64::from(hex_value(*byte)));
        }

        return Some(TokenKind::Int(value as i64));
    }

    hexadecimal_float(text).map(TokenKind::Float)
}

/// `1.8p3` and friends, after the `0x`.
fn hexadecimal_float(text: &[u8]) -> Option<f64> {
    let mut mantissa: u64 = 0;
    let mut scale: i32 = 0;
    let mut sticky = false;
    let mut digits = 0;
    let mut fraction = false;
    let mut at = 0;

    while at < text.len() {
        let byte = text[at];
        if byte == b'.' {
            if fraction {
                return None;
            }

            fraction = true;
        } else if byte.is_ascii_hexdigit() {
            digits += 1;

            if mantissa <= u64::MAX >> 4 {
                mantissa = (mantissa << 4) | u64::from(hex_value(byte));
                if fraction {
                    scale -= 4;
                }
            } else {
                // Out of room.
                sticky |= byte != b'0';

                if !fraction {
                    scale += 4;
                }
            }
        } else {
            break;
        }

        at += 1;
    }

    if digits == 0 {
        return None;
    }

    if at < text.len() {
        scale = scale.checked_add(binary_exponent(&text[at..])?)?;
    }

    Some(scaled(mantissa, scale, sticky))
}

/// The `p` exponent.
fn binary_exponent(text: &[u8]) -> Option<i32> {
    if !matches!(text.first(), Some(b'p' | b'P')) {
        return None;
    }

    let mut at = 1;
    let negative = match text.get(at) {
        Some(b'+') => {
            at += 1;
            false
        }
        Some(b'-') => {
            at += 1;
            true
        }
        _ => false,
    };

    if at == text.len() {
        return None;
    }

    let mut value: i32 = 0;
    for byte in &text[at..] {
        if !byte.is_ascii_digit() {
            return None;
        }

        value = (value * 10 + i32::from(byte - b'0')).min(1 << 20);
    }

    Some(if negative { -value } else { value })
}

/// `mantissa * 2^scale` as the nearest `f64`, ties to even.
/// `sticky` says whether something nonzero was already dropped below the mantissa.
fn scaled(mantissa: u64, scale: i32, sticky: bool) -> f64 {
    if mantissa == 0 {
        return 0.0;
    }

    let mut mantissa = mantissa;
    let mut exponent = scale;
    let mut sticky = sticky;
    let mut round;

    // Bring it to exactly 53 significant bits, the most an `f64` carries.
    let bits = 64 - mantissa.leading_zeros() as i32;
    if bits > 53 {
        let (value, dropped, lost) = drop_low_bits(mantissa, (bits - 53) as u32);
        mantissa = value;
        round = dropped;
        sticky |= lost;
        exponent += bits - 53;
    } else {
        mantissa <<= (53 - bits) as u32;
        exponent -= 53 - bits;
        round = false;
    }

    // Below the smallest normal the exponent cannot fall any further, so precision goes instead.
    if exponent < -1074 {
        sticky |= round;

        let (value, dropped, lost) = drop_low_bits(mantissa, (-1074 - exponent) as u32);
        mantissa = value;
        round = dropped;
        sticky |= lost;
        exponent = -1074;
    }

    if round && (sticky || mantissa & 1 == 1) {
        mantissa += 1;
        if mantissa >> 53 != 0 {
            mantissa >>= 1;
            exponent += 1;
        }
    }

    if mantissa == 0 {
        return 0.0;
    }

    // 53 bits means there is a leading 1 to hide.
    // Fewer means the value is subnormal, and thent the bit pattern is the mantissa as it stands.
    if 64 - mantissa.leading_zeros() == 53 {
        let field = exponent + 52 + 1023;
        if field >= 0x7ff {
            return f64::INFINITY;
        }

        return f64::from_bits(((field as u64) << 52) | (mantissa & ((1 << 52) - 1)));
    }

    f64::from_bits(mantissa)
}

/// Drop `count` low bits, reporting the top one dropped - which decides the rounding - and
/// whether any of the rest was set, which breaks a tie.
fn drop_low_bits(value: u64, count: u32) -> (u64, bool, bool) {
    match count {
        0 => (value, false, false),
        65.. => (0, false, value != 0),
        64 => (0, value >> 63 & 1 == 1, value & ((1 << 63) - 1) != 0),
        _ => (
            value >> count,
            value >> (count - 1) & 1 == 1,
            count > 1 && value & ((1 << (count - 1)) - 1) != 0,
        ),
    }
}

/// ASCII letters and `_`, and nothing else - a byte above 127 is a token of its own, not part of a name.
fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
