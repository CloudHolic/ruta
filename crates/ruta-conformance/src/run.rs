//! Running both interpreters on the same input and comparing what comes out.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::manifest::{Case, Recipe};
use crate::outcome::{Comparison, Outcome, Streams};
use crate::sandbox::{self, Sandbox, copy_dir};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const SCRIPT_NAME: &str = "script.lua";
const DRIVER_NAME: &str = "_driver.lua";

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
            Sandbox::Fresh,
        )
    }

    /// Compare the two interpreters parsing one file, without running it.
    pub fn parse_file(&self, source: &Path) -> Result<Comparison> {
        self.compare(
            &mut |dir| {
                std::fs::copy(source, dir.join(SCRIPT_NAME))
                    .with_context(|| format!("cannot copy {}", source.display()))?;
                Ok(vec![OsString::from("-p"), OsString::from(SCRIPT_NAME)])
            },
            Streams::All,
            Sandbox::Reused,
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
                Streams::Shape
            } else {
                Streams::All
            },
            Sandbox::Fresh,
        )
    }

    fn run_side(
        &self,
        program: &Path,
        side: &str,
        prepare: &mut dyn FnMut(&Path) -> Result<Vec<OsString>>,
        mode: Sandbox,
    ) -> Result<Option<Outcome>> {
        let dir = sandbox::sandbox(&self.workdir, side, mode)?;
        let args = prepare(&dir)?;

        sandbox::run(program, &dir, &args, self.timeout)
    }

    fn compare(
        &self,
        prepare: &mut dyn FnMut(&Path) -> Result<Vec<OsString>>,
        streams: Streams,
        sandbox: Sandbox,
    ) -> Result<Comparison> {
        let reference = self.run_side(&self.reference, "reference", prepare, sandbox)?;
        let candidate = self.run_side(&self.candidate, "candidate", prepare, sandbox)?;

        let (Some(reference), Some(candidate)) = (reference, candidate) else {
            return Ok(Comparison::Timeout);
        };

        let same = reference.agrees_with(&candidate, streams);

        if same {
            Ok(Comparison::Match)
        } else {
            Ok(Comparison::Mismatch {
                reference,
                candidate,
            })
        }
    }
}

/// The way `all.lua` runs `big.lua`.
fn coroutine_driver(name: &str) -> String {
    format!(
        "local f = coroutine.wrap(assert(loadfile('{name}')))\n\
            assert(f() == 'b')\n\
            assert(f() == 'a')\n"
    )
}
