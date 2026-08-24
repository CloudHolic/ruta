//! Running both interpreters on the same input and comparing what comes out.

use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::manifest::{Case, Recipe};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const SCRIPT_NAME: &str = "script.lua";
const DRIVER_NAME: &str = "_driver.lua";
const INTERPRETER_TOKEN: &[u8] = b"<interpreter>";

/// What one interpreter produced.
#[derive(Debug)]
pub struct Outcome {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: Option<i32>,
}

/// Which streams a case is compared on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Streams {
    /// stdout, stderr and the exit code.
    All,
    /// The exit code alone, for files whose output is not reproducible run to run.
    StatusOnly,
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

/// A pair of interpreters and the directory their sandboxes live in.
#[derive(Debug)]
pub struct Harness {
    reference: PathBuf,
    candidate: PathBuf,
    workdir: PathBuf,
    timeout: Duration,
}

impl Harness {
    pub fn new(reference: PathBuf, candidate: PathBuf, workdir: PathBuf) -> Self {
        Self {
            reference,
            candidate,
            workdir,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Compare the two interpreters on a script given as a string.
    pub fn differential(&self, script: &str) -> Result<Comparison> {
        self.compare(
            &mut |dir| {
                std::fs::write(dir.join(SCRIPT_NAME), script)
                    .with_context(|| format!("cannot write {SCRIPT_NAME}"))?;
                Ok(vec![OsString::from(SCRIPT_NAME)])
            },
            Streams::All,
        )
    }

    /// Compare the two interpreters on one manifest case.
    pub fn run_case(&self, case: &Case, suite_dir: &Path) -> Result<Comparison> {
        self.compare(
            &mut |dir| {
                copy_dir(suite_dir, dir)?;

                let mut args = Vec::new();
                if case.port {
                    args.push(OsString::from("-e"));
                    args.push(OsString::from("_port=true"));
                }

                match case.recipe {
                    Some(Recipe::Coroutine) => {
                        std::fs::write(dir.join(DRIVER_NAME), coroutine_driver(&case.name))
                            .with_context(|| format!("cannot write {DRIVER_NAME}"))?;
                        args.push(OsString::from(DRIVER_NAME));
                    }
                    None => args.push(OsString::from(&case.name)),
                }
                Ok(args)
            },
            if case.nondeterministic {
                Streams::StatusOnly
            } else {
                Streams::All
            },
        )
    }

    fn compare(
        &self,
        prepare: &mut dyn FnMut(&Path) -> Result<Vec<OsString>>,
        streams: Streams,
    ) -> Result<Comparison> {
        let reference = self.run_side(&self.reference, "reference", prepare)?;
        let candidate = self.run_side(&self.candidate, "candidate", prepare)?;

        let (Some(reference), Some(candidate)) = (reference, candidate) else {
            return Ok(Comparison::Timeout);
        };

        let same = match streams {
            Streams::All => {
                reference.stdout == candidate.stdout
                    && reference.stderr == candidate.stderr
                    && reference.code == candidate.code
            }
            Streams::StatusOnly => reference.code == candidate.code,
        };

        if same {
            Ok(Comparison::Match)
        } else {
            Ok(Comparison::Mismatch {
                reference,
                candidate,
            })
        }
    }

    /// `None` means the process was killed for excedding the timeout.
    fn run_side(
        &self,
        program: &Path,
        side: &str,
        prepare: &mut dyn FnMut(&Path) -> Result<Vec<OsString>>,
    ) -> Result<Option<Outcome>> {
        let dir = self.fresh_sandbox(side)?;
        let args = prepare(&dir)?;

        let stdout_path = dir.join(".stdout");
        let stderr_path = dir.join(".stderr");

        // Output goes to files rather than pipes: reading a pipe blocks when the child hangs,
        // which would deadlock against the timeout below.
        let mut child = Command::new(program)
            .args(&args)
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(File::create(&stdout_path)?)
            .stderr(File::create(&stderr_path)?)
            .spawn()
            .with_context(|| format!("cannot run {}", program.display()))?;

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break Some(status);
            }

            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }

            std::thread::sleep(POLL_INTERVAL);
        };

        let Some(status) = status else {
            return Ok(None);
        };

        Ok(Some(Outcome {
            stdout: strip_interpreter_path(std::fs::read(&stdout_path)?, program),
            stderr: strip_interpreter_path(std::fs::read(&stderr_path)?, program),
            code: status.code(),
        }))
    }

    /// Each side gets its own directory, wiped before every run. The suite writes into its working directory,
    /// so a shared or reused one would let an earlier run change a later result.
    fn fresh_sandbox(&self, side: &str) -> Result<PathBuf> {
        let dir = self.workdir.join(side);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .with_context(|| format!("cannot clear {}", dir.display()))?;
        }

        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;

        Ok(dir)
    }
}

/// Lua's standalone interpreter prefixes its error messages with `argv[0]`, which is where
/// each binary happens to live. So replace with a fixed token on both sides.
fn strip_interpreter_path(bytes: Vec<u8>, program: &Path) -> Vec<u8> {
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

/// The way `all.lua` runs `big.lua`.
fn coroutine_driver(name: &str) -> String {
    format!(
        "local f = coroutine.wrap(assert(loadfile('{name}')))\n\
            assert(f() == 'b')\n\
            assert(f() == 'a')\n"
    )
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("cannot create {}", to.display()))?;

    for entry in
        std::fs::read_dir(from).with_context(|| format!("cannot read {}", from.display()))?
    {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            std::fs::copy(&source, &target)
                .with_context(|| format!("cannot copy {}", source.display()))?;
        }
    }

    Ok(())
}
