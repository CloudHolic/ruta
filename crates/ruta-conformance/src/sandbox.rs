//! Where a case runs: the throwaway directory, and one interpreter inside it under a timeout.

use std::ffi::OsString;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::outcome::{Outcome, strip_interpreter_path};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Whether a case needs a clean working directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sandbox {
    Fresh,
    Reused,
}

/// Each side gets its own directory. `Sandbox::Fresh` wipes it first: the suite writes into its working directory,
/// so a shared or reused one would let an earlier run change a later result.
pub(crate) fn sandbox(workdir: &Path, side: &str, mode: Sandbox) -> Result<PathBuf> {
    let dir = workdir.join(side);
    if mode == Sandbox::Fresh && dir.exists() {
        fs::remove_dir_all(&dir).with_context(|| format!("cannot clear {}", dir.display()))?;
    }
    fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;

    Ok(dir)
}

/// Run one interpreter in `dir`. `None` means it was killed for excedding `timeout`.
pub(crate) fn run(
    program: &Path,
    dir: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<Option<Outcome>> {
    let stdout_path = dir.join(".stdout");
    let stderr_path = dir.join(".stderr");

    // Output goes to files rather than pipes: reading a pipe blocks when the child hangs,
    // which would deadlock against the timeout below.
    let mut child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(File::create(&stdout_path)?)
        .stderr(File::create(&stderr_path)?)
        .spawn()
        .with_context(|| format!("cannot run {}", program.display()))?;

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }

        thread::sleep(POLL_INTERVAL);
    };

    let Some(status) = status else {
        return Ok(None);
    };

    Ok(Some(Outcome {
        stdout: strip_interpreter_path(fs::read(&stdout_path)?, program),
        stderr: strip_interpreter_path(fs::read(&stderr_path)?, program),
        code: status.code(),
    }))
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("cannot create {}", to.display()))?;

    for entry in fs::read_dir(from).with_context(|| format!("cannot read {}", from.display()))? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .with_context(|| format!("cannot copy {}", source.display()))?;
        }
    }

    Ok(())
}
