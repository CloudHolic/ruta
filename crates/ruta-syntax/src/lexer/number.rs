//! Numerals: what a scan takes in, and what the bytes turn out to be worth.

use crate::error::{Error, ErrorKind, Near};
use crate::token::{Token, TokenKind};

use super::Lexer;
use super::bytes::{hex_value, is_name_start};

/// A number as a string spells it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl<'a> Lexer<'a> {
    pub(super) fn read_numeral(&mut self, start: usize) -> Result<Token<'a>, Error> {
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
            None => Err(Error {
                kind: ErrorKind::MalformedNumber,
                at: start as u32,
                near: Near::Buffer(text.to_vec()),
            }),
        }
    }
}

/// Reads a whole string as a number, the way a numeral in source reads.
pub fn parse_number(text: &[u8]) -> Option<Number> {
    let text = trim(text);
    let (negative, digits) = match text {
        [b'-', rest @ ..] => (true, rest),
        [b'+', rest @ ..] => (false, rest),
        _ => (false, text),
    };

    // A numeral starts with a digit or a point, which also turns away `inf` and `nan`.
    if !digits
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
    {
        return None;
    }

    if negative
        && digits.iter().all(u8::is_ascii_digit)
        && str::from_utf8(digits).ok()?.parse::<u64>() == Ok(1 << 63)
    {
        return Some(Number::Int(i64::MIN));
    }

    let number = match numeral_value(digits)? {
        TokenKind::Int(value) => Number::Int(value),
        TokenKind::Float(value) => Number::Float(value),
        other => unreachable!("a numeral that is not a number: {other:?}"),
    };

    Some(match (negative, number) {
        (false, number) => number,
        (true, Number::Int(value)) => Number::Int(value.wrapping_neg()),
        (true, Number::Float(value)) => Number::Float(-value),
    })
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

fn trim(text: &[u8]) -> &[u8] {
    let space = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r');
    let start = text
        .iter()
        .position(|byte| !space(byte))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|byte| !space(byte))
        .map_or(start, |at| at + 1);

    &text[start..end]
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
