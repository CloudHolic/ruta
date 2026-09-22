//! What a run produced, and what it means for two runs to agree.

use std::path::Path;

const INTERPRETER_TOKEN: &[u8] = b"<interpreter>";

/// What one interpreter produced.
#[derive(Debug)]
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: Option<i32>,
}

impl Outcome {
    /// Whether two runs agree on the streams this case is compared on.
    pub(crate) fn agrees_with(&self, other: &Outcome, streams: Streams) -> bool {
        match streams {
            Streams::All => {
                self.stdout == other.stdout
                    && self.stderr == other.stderr
                    && self.code == other.code
            }
            Streams::Shape => {
                self.code == other.code
                    && line_count(&self.stdout) == line_count(&other.stdout)
                    && line_count(&self.stderr) == line_count(&other.stderr)
            }
        }
    }
}

/// Which streams a case is compared on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Streams {
    /// stdout, stderr and the exit code.
    All,
    /// The exit code and the number of lines on each stream, for files whose output is not
    /// reproducible run to run.
    Shape,
}

/// The result of running both interpreters on the same input.
#[derive(Debug)]
pub enum Comparison {
    Match,
    Mismatch {
        reference: Outcome,
        candidate: Outcome,
    },
    Timeout,
}

/// Lua's standalone interpreter prefixes its error messages with `argv[0]`, which is where
/// each binary happens to live. So replace with a fixed token on both sides.
pub(crate) fn strip_interpreter_path(bytes: Vec<u8>, program: &Path) -> Vec<u8> {
    let needle = program.to_string_lossy();
    let needle = needle.as_bytes();
    if needle.is_empty() || !bytes.windows(needle.len()).any(|w| w == needle) {
        return bytes;
    }

    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes.as_slice();

    while let Some(at) = rest.windows(needle.len()).position(|w| w == needle) {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(INTERPRETER_TOKEN);
        rest = &rest[at + needle.len()..];
    }

    out.extend_from_slice(rest);
    out
}

/// What a program built on the platform's C runtime wrote, before that runtime changed it on the way out.
/// `ruta` writes its bytes as they are, so this is what it is compared against.
pub(crate) fn as_written(bytes: Vec<u8>) -> Vec<u8> {
    match cfg!(windows) {
        true => undo_nan_spelling(undo_text_mode(bytes)),
        false => bytes,
    }
}

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

/// A text-mode stream puts a `\r` in front of every `\n` and changes nothing else,
/// so taking it back off every `\r\n` recovers the bytes exactly.
fn undo_text_mode(bytes: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());

    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b'\r' && bytes.get(at + 1) == Some(&b'\n') {
            continue;
        }

        out.push(*byte);
    }

    out
}

/// The MSVC runtime spells a NaN `-nan(ind)` or `nan(snan)` where others write `-nan` and `nan`.
/// Unlike the line endings this is a guess; a program that prints those words itself is rewritten too.
fn undo_nan_spelling(bytes: Vec<u8>) -> Vec<u8> {
    const SUFFIXES: [&[u8]; 2] = [b"(ind)", b"(snan)"];

    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes.as_slice();

    while let Some(at) = rest.windows(3).position(|window| window == b"nan") {
        out.extend_from_slice(&rest[..at + 3]);
        rest = &rest[at + 3..];

        if let Some(suffix) = SUFFIXES.iter().find(|suffix| rest.starts_with(suffix)) {
            rest = &rest[suffix.len()..];
        }
    }

    out.extend_from_slice(rest);
    out
}
