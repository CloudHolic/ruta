//! Differential test scoreboard.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use ruta_conformance::manifest::{Manifest, Tier};
use ruta_conformance::run::{Comparison, Harness};

const TIER_ORDER: [Tier; 3] = [Tier::V1, Tier::V2, Tier::Impossible];

fn main() -> ExitCode {
    // A mismatch is progress information, not a failure: every case is expected to fail
    // until the interpreter exists. Only a broken setup earns a non-zero exit.
    match report() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conformance: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn report() -> Result<()> {
    let root = repo_root();
    let reference = root
        .join("target")
        .join("reference")
        .join(if cfg!(windows) { "lua.exe" } else { "lua" });
    if !reference.exists() {
        bail!(
            "no reference interpreter at {} - run `cargo xtask build-reference`",
            reference.display()
        );
    }

    let suite = root.join("vendor").join("lua-tests");
    let manifest = Manifest::load(&root.join("conformance").join("manifest.toml"), &suite)
        .context("loading the manifest")?;
    let harness = Harness::new(
        reference,
        PathBuf::from(env!("CARGO_BIN_EXE_ruta")),
        root.join("target").join("conformance"),
    );

    let mut matched = [0usize; TIER_ORDER.len()];
    let mut total = [0usize; TIER_ORDER.len()];

    for case in manifest.cases() {
        let index = TIER_ORDER
            .iter()
            .position(|tier| *tier == case.tier)
            .expect("every tier appears in TIER_ORDER");
        total[index] += 1;

        let status = match harness.run_case(case, &suite)? {
            Comparison::Match => {
                matched[index] += 1;
                "ok"
            }
            Comparison::Mismatch { .. } => "FAIL",
            Comparison::Timeout => "TIME",
        };
        println!("  {status:<6}{}", case.name);
    }

    print_scoreboard(&matched, &total, manifest.skipped().count());
    Ok(())
}

fn print_scoreboard(matched: &[usize], total: &[usize], skipped: usize) {
    println!("\nruta conformance - Lua 5.5.1\n");

    let mut counted = 0;
    let mut counted_total = 0;
    for (index, tier) in TIER_ORDER.iter().enumerate() {
        if *tier == Tier::Impossible {
            // These cannot pass, so a fraction would be misleading.
            println!("  {:<12}-/{}", label(*tier), total[index]);
            continue;
        }

        counted += matched[index];
        counted_total += total[index];
        println!("  {:<12}{}/{}", label(*tier), matched[index], total[index]);
    }

    println!("\n  {:<12}{counted}/{counted_total}", "total");
    println!("  {:<12}{skipped}", "skipped");
}

fn label(tier: Tier) -> &'static str {
    match tier {
        Tier::V1 => "v1.0",
        Tier::V2 => "v2.0",
        Tier::Impossible => "impossible",
    }
}

/// The repository root.
fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/ruta-cli/ is two levels below the repository root")
}
