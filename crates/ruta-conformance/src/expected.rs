//! The corpus mismatches that are supposed to be there, and the stage that closes each.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// A corpus entry that is expected to disagree with the reference, and why.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The stage that closes the gap.
    pub stage: u32,
    /// What the reference says, with the chunk name and line elided.
    pub error: String,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct Expected {
    cases: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    cases: BTreeMap<String, Entry>,
}

impl Expected {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let raw: RawFile =
            toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))?;

        Ok(Self { cases: raw.cases })
    }

    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.cases.get(key)
    }

    pub fn len(&self) -> usize {
        self.cases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }

    /// An entry naming a file the corpus no longer holds would sit here forever, excusing nothing.
    pub fn check_against_corpus(&self, keys: &BTreeSet<String>) -> Result<()> {
        let ghosts: Vec<&str> = self
            .cases
            .keys()
            .map(String::as_str)
            .filter(|key| !keys.contains(*key))
            .collect();

        if !ghosts.is_empty() {
            bail!(
                "listed in parse-expected.toml but absent from the corpus: {}",
                ghosts.join(", ")
            );
        }

        Ok(())
    }
}
