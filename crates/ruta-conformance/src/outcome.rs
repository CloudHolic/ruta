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

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}
