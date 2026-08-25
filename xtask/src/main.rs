//! Repository maintenance tasks.
//!
//! Run as `cargo xtask <command>`.

use std::path::Path;

use anyhow::{Result, bail};

use crate::reference::Driver;

mod corpus;
mod reference;

const USAGE: &str = "usage: cargo xtask <build-reference [--luac] | extract-parse-corpus>";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("build-reference") => {
            let driver = match args.next().as_deref() {
                None => Driver::Lua,
                Some("--luac") => Driver::Luac,
                Some(other) => bail!("unknown option `{other}`\n{USAGE}"),
            };

            if let Some(extra) = args.next() {
                bail!("unexpected argument `{extra}`\n{USAGE}");
            }
            reference::build(driver)
        }
        Some("extract-parse-corpus") => corpus::extract(),
        Some(other) => bail!("unknown command: `{other}`\n{USAGE}"),
        None => bail!("{USAGE}"),
    }
}

/// Root of the repository - the parent of `xtask/`.
pub(crate) fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/ is under the root of the repository")
}
