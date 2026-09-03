//! Where the repository is, from a test binary's point of view.

use std::path::Path;

pub(crate) fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/ruta-cli/ is two levels below the repository root")
}
