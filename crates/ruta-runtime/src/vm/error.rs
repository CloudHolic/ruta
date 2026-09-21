//! What a running program throws.

use crate::value::Value;

/// A value on its way out of the program.
/// Lua errors are values, not a Rust error type: `error({code = 1})` has to survive the trip.
#[derive(Debug)]
pub struct Error {
    pub value: Value,
}

/// The most a message shows of a chunk's name.
const SHOWN: usize = 59;

/// How a chunk names itself in a message. A leading `=` means the rest is the name as written;
/// a leading `@` means a file, which is trimmed from the front so the tail shows.
pub(super) fn chunk(name: &[u8]) -> Vec<u8> {
    let (mark, rest) = name.split_first().expect("a name the loader wrote");

    debug_assert!(
        *mark == b'=' || *mark == b'@',
        "only named chunks reach teh runtime before `load` exists"
    );

    if rest.len() <= SHOWN {
        return rest.to_vec();
    }

    match mark {
        b'@' => {
            let mut out = b"...".to_vec();
            out.extend_from_slice(&rest[rest.len() - (SHOWN - 3)..]);

            out
        }
        _ => rest[..SHOWN].to_vec(),
    }
}
