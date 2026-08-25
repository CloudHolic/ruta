//! Parse-only scoreboard: `luac -p` against `ruta -p`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use ruta_conformance::manifest::Manifest;
use ruta_conformance::outcome::Comparison;
use ruta_conformance::run::Harness;

mod common;

use common::repo_root;

fn main() -> ExitCode {
    match report() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("parse: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn report() -> Result<()> {
    let root = repo_root();
    let luac = root
        .join("target")
        .join("reference")
        .join(if cfg!(windows) { "luac.exe" } else { "luac" });

    if !luac.exists() {
        bail!(
            "no reference compiler at {} - run `cargo xtask build-reference --luac`",
            luac.display()
        );
    }

    let suite = root.join("vendor").join("lua-tests");
    let manifest = Manifest::load(&root.join("conformance").join("manifest.toml"), &suite)
        .context("loading the manifest")?;
    let harness = Harness::new(
        luac,
        PathBuf::from(env!("CARGO_BIN_EXE_ruta")),
        root.join("target").join("parse"),
    );

    // Every file in the suite, including the three the conformance board skips.
    let mut names: Vec<&str> = manifest
        .cases()
        .map(|case| case.name.as_str())
        .chain(manifest.skipped().map(|skipped| skipped.name.as_str()))
        .collect();
    names.sort_unstable();

    let mut matched = 0;
    for name in &names {
        let status = match harness.parse_file(&suite.join(name))? {
            Comparison::Match => {
                matched += 1;
                "ok"
            }
            Comparison::Mismatch { .. } => "FAIL",
            Comparison::Timeout => "TIME",
        };

        println!("  {status:<6}{name}");
    }

    let conformance = root.join("conformance");
    let mut corpus = collect_corpus(&conformance.join("parse-corpus"))?;
    corpus.extend(collect_corpus(&conformance.join("parse-cases"))?);
    corpus.sort();

    let mut corpus_matched = 0;

    for path in &corpus {
        if let Comparison::Match = harness.parse_file(path)? {
            corpus_matched += 1;
        }
    }

    print_scoreboard(matched, names.len(), corpus_matched, corpus.len());
    Ok(())
}

fn collect_corpus(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        if !current.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&current)
            .with_context(|| format!("cannot read {}", current.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "lua") {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn print_scoreboard(files: usize, files_total: usize, corpus: usize, corpus_total: usize) {
    println!("\nruta parse - Lua 5.5.1\n");
    println!("  {:<13}{files}/{files_total}", "files");
    println!("  {:<13}{corpus}/{corpus_total}", "corpus");
    println!(
        "\n  {:<13}{}/{}",
        "total",
        files + corpus,
        files_total + corpus_total
    );
}
