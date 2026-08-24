//! Manifest schema, loading and validation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Scoreboard group for a test file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Tier {
    #[serde(rename = "v1.0")]
    V1,
    #[serde(rename = "v2.0")]
    V2,
    #[serde(rename = "impossible")]
    Impossible,
}

/// A non-default way of running a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Recipe {
    /// Wrap the file in `coroutine.wrap`, the way `all.lua` runs `big.lua`.
    #[serde(rename = "coroutine")]
    Coroutine,
}

/// A file that counts toward the scoreboard.
#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub tier: Tier,
    pub port: bool,
    pub recipe: Option<Recipe>,
    pub note: Option<String>,
}

/// A file deliberately left out of the scoreboard.
#[derive(Debug)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct Manifest {
    cases: Vec<Case>,
    skipped: Vec<Skipped>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    files: BTreeMap<String, RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    tier: Option<Tier>,
    skip: Option<String>,
    #[serde(default)]
    port: bool,
    recipe: Option<Recipe>,
    note: Option<String>,
}

impl Manifest {
    /// Load the manifest and check it describes exactly the `.lua` files in `suite_dir`.
    pub fn load(path: &Path, suite_dir: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let raw: RawManifest =
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;

        let mut cases = Vec::new();
        let mut skipped = Vec::new();
        for (name, entry) in raw.files {
            match (entry.tier, entry.skip) {
                (Some(_), Some(_)) => bail!("{name}: `tier` and `skip` are mutually exclusive"),
                (None, None) => bail!("{name}: one of `tier` or `skip` is required"),
                (Some(tier), None) => cases.push(Case {
                    name,
                    tier,
                    port: entry.port,
                    recipe: entry.recipe,
                    note: entry.note,
                }),
                (None, Some(reason)) => skipped.push(Skipped { name, reason }),
            }
        }

        let manifest = Self { cases, skipped };
        manifest.check_against_suite(suite_dir)?;
        Ok(manifest)
    }

    /// Files that count toward the scoreboard.
    pub fn cases(&self) -> impl Iterator<Item = &Case> {
        self.cases.iter()
    }

    /// Files deliberately left out of the scoreboard.
    pub fn skipped(&self) -> impl Iterator<Item = &Skipped> {
        self.skipped.iter()
    }

    /// A file missing from the manifest would be silently left uncounted, and an entry for
    /// a file that no longer exists would be counted as a permanent failure.
    /// Both are errors.
    fn check_against_suite(&self, suite_dir: &Path) -> Result<()> {
        let mut on_disk = BTreeSet::new();
        for entry in std::fs::read_dir(suite_dir)
            .with_context(|| format!("cannot read {}", suite_dir.display()))?
        {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "lua") {
                let name = path
                    .file_name()
                    .expect("read_dir yields named entries")
                    .to_string_lossy()
                    .into_owned();
                on_disk.insert(name);
            }
        }

        let listed: BTreeSet<&str> = self
            .cases
            .iter()
            .map(|c| c.name.as_str())
            .chain(self.skipped.iter().map(|s| s.name.as_str()))
            .collect();

        let unlisted: Vec<&str> = on_disk
            .iter()
            .map(String::as_str)
            .filter(|name| !listed.contains(name))
            .collect();
        if !unlisted.is_empty() {
            bail!("not listed in the manifest: {}", unlisted.join(", "));
        }

        let ghosts: Vec<&str> = listed
            .iter()
            .copied()
            .filter(|name| !on_disk.contains(*name))
            .collect();
        if !ghosts.is_empty() {
            bail!(
                "listesd in the manifest but absent from {}: {}",
                suite_dir.display(),
                ghosts.join(", ")
            );
        }

        Ok(())
    }
}
