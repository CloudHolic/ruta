//! Extracting the parse corpus: the strings the official suite passes to `load`.

use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use ruta_conformance::manifest::Manifest;
use ruta_conformance::sandbox::copy_dir;

use crate::repo_root;

const MAX_CHUNK: usize = 4096;
const MAX_PER_SOURCE: usize = 100;

/// Collect the strings the suite passes to `load` into `conformance/parse-corpus/`.
pub(crate) fn extract() -> Result<()> {
    let root = repo_root();
    let lua = root
        .join("target")
        .join("reference")
        .join(if cfg!(windows) { "lua.exe" } else { "lua" });

    if !lua.exists() {
        bail!(
            "no reference interpreter at {} - run `cargo xtask build-reference`",
            lua.display()
        );
    }

    let suite = root.join("vendor").join("lua-tests");
    let manifest = Manifest::load(&root.join("conformance").join("manifest.toml"), &suite)
        .context("loading the manifest")?;
    let prelude = root.join("conformance").join("extract-loads.lua");
    let workdir = root.join("target").join("parse-corpus-extract");
    let captures = root.join("target").join("parse-capture");
    let out_root = root.join("conformance").join("parse-corpus");

    reset_dir(&workdir)?;
    reset_dir(&captures)?;
    reset_dir(&out_root)?;
    copy_dir(&suite, &workdir)?;

    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    let mut kept_total = 0;

    for case in manifest.cases().filter(|case| case.name != "all.lua") {
        let stem = case.name.trim_end_matches(".lua");
        let capture = captures.join(format!("{stem}.bin"));

        let mut command = Command::new(&lua);
        command
            .current_dir(&workdir)
            .env("RUTA_CAPTURE", &capture)
            .arg("-e")
            .arg(format!("dofile[[{}]]", prelude.display()));

        if case.port {
            command.arg("-e").arg("_port=true");
        }

        command
            .arg(&case.name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("cannot run the reference on {}", case.name))?;

        let kept = write_corpus(&capture, &out_root.join(stem), &mut seen)?;
        kept_total += kept;
        println!("  {:<18}{kept}", case.name);
    }

    println!("\n{kept_total} chunks in {}", out_root.display());
    Ok(())
}

/// Split one capture file into chunks and write the ones worth keeping.
fn write_corpus(capture: &Path, out_dir: &Path, seen: &mut HashSet<Vec<u8>>) -> Result<usize> {
    let Ok(data) = std::fs::read(capture) else {
        return Ok(0);
    };

    let mut kept = 0;
    let mut rest = data.as_slice();

    while kept < MAX_PER_SOURCE {
        let Some(newline) = rest.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let len: usize = std::str::from_utf8(&rest[..newline])
            .ok()
            .and_then(|s| s.parse().ok())
            .with_context(|| format!("malformed framing in {}", capture.display()))?;

        rest = &rest[newline + 1..];
        if rest.len() < len {
            bail!("truncated chunk in {}", capture.display());
        }

        let (chunk, remainder) = rest.split_at(len);
        rest = remainder;

        // A leading ESC means bytecode rather than source.
        let is_binary = chunk.first() == Some(&0x1B);
        let truncates = chunk.contains(&0x1A);
        if chunk.len() > MAX_CHUNK || is_binary || truncates || !seen.insert(chunk.to_vec()) {
            continue;
        }

        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("cannot create {}", out_dir.display()))?;
        kept += 1;

        let path = out_dir.join(format!("{kept:03}.lua"));
        std::fs::write(&path, chunk).with_context(|| format!("cannot write {}", path.display()))?;
    }

    Ok(kept)
}

fn reset_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).with_context(|| format!("cannot clear {}", dir.display()))?;
    }

    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    Ok(())
}
