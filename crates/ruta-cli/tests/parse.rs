//! Parse-only scoreboard: `luac -p` against `ruta -p`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use ruta_conformance::expected::Expected;
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
    let expected = Expected::load(&conformance.join("parse-expected.toml"))
        .context("loading the expected-mismatch list")?;
    let mut corpus = collect_corpus(&conformance.join("parse-corpus"))?;
    corpus.extend(collect_corpus(&conformance.join("parse-cases"))?);
    corpus.sort();

    expected.check_against_corpus(&corpus.iter().map(|(key, _)| key.clone()).collect())?;

    let mut corpus_matched = 0;
    let mut known = 0;
    let mut unexpected = Vec::new();
    let mut resolved = Vec::new();

    for (key, path) in &corpus {
        let listed = expected.get(key);

        match harness.parse_file(path)? {
            Comparison::Match => {
                corpus_matched += 1;
                if listed.is_some() {
                    resolved.push(key.as_str());
                }
            }
            outcome => match listed {
                Some(_) => known += 1,
                None => unexpected.push((key.as_str(), outcome)),
            },
        }
    }

    print_scoreboard(
        matched,
        names.len(),
        corpus_matched,
        corpus.len(),
        known,
        unexpected.len(),
    );

    if !unexpected.is_empty() {
        println!("\nunexpected mismatches:\n");
        for (key, outcome) in &unexpected {
            match outcome {
                Comparison::Mismatch {
                    reference,
                    candidate,
                } => {
                    println!("  {key}");
                    println!("    reference  {}", first_line(&reference.stderr));
                    println!("    ruta       {}", first_line(&candidate.stderr));
                }
                Comparison::Timeout => println!("  {key}  (timeout)"),
                Comparison::Match => unreachable!("matches are not collected here"),
            }
        }
    }

    if !resolved.is_empty() {
        println!("\nnow matching - remove from conformance/parse-expected.toml:\n");
        for key in &resolved {
            println!("  {key}");
        }
    }

    Ok(())
}

fn collect_corpus(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(current) = stack.pop() {
        if !current.exists() {
            continue;
        }

        for entry in
            fs::read_dir(&current).with_context(|| format!("cannot read {}", current.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "lua") {
                let key = path
                    .strip_prefix(root)
                    .expect("walked down from root")
                    .to_string_lossy()
                    .replace('\\', "/");

                files.push((key, path));
            }
        }
    }

    files.sort();
    Ok(files)
}

fn first_line(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.lines().next().unwrap_or("(no output)").to_string()
}

fn print_scoreboard(
    files: usize,
    files_total: usize,
    corpus: usize,
    corpus_total: usize,
    known: usize,
    unexpected: usize,
) {
    println!("\nruta parse - Lua 5.5.1\n");
    println!("  {:<13}{files}/{files_total}", "files");
    println!(
        "  {:<13}{corpus}/{corpus_total}   ({known} known, {unexpected} unexpected)",
        "corpus"
    );
    println!(
        "\n  {:<13}{}/{}",
        "total",
        files + corpus,
        files_total + corpus_total
    );
}
