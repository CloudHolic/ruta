//! What a byte counts as.

/// The byte must already be a hex digit.
pub(super) fn hex_value(byte: u8) -> u32 {
    if byte.is_ascii_digit() {
        u32::from(byte - b'0')
    } else {
        u32::from(byte.to_ascii_lowercase() - b'a') + 10
    }
}

/// ASCII letters and `_`, and nothing else - a byte above 127 is a token of its own, not part of a name.
pub(super) fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

pub(super) fn is_name_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
