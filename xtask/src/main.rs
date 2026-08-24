//! Repository maintenance tasks.
//!
//! Run as `cargo xtask <command>`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

const USAGE: &str = "usage: cargo xtask build-reference";

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("build-reference") => build_reference(),
        Some(other) => bail!("unknown command `{other}`\n{USAGE}"),
        None => bail!("{USAGE}"),
    }
}

/// Root of the repository - the parent of `xtask/`.
fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/ is under the root of the repository")
}

/// Build PUC-Lua to produce `target/reference/lua[.exe]`
fn build_reference() -> Result<()> {
    let root = repo_root();
    let src = root.join("vendor").join("puc-lua").join("src");
    let out_dir = root.join("target").join("reference");
    let obj_dir = out_dir.join("obj");
    let exe = out_dir.join(if cfg!(windows) { "lua.exe" } else { "lua" });

    if is_up_to_date(&exe, &src)? {
        println!("reference up to date: {}", exe.display());
        return Ok(());
    }

    let sources = lua_sources(&src)?;
    std::fs::create_dir_all(&obj_dir)
        .with_context(|| format!("cannot create {}", obj_dir.display()))?;

    let triple = host_triple()?;
    let mut build = cc::Build::new();
    build
        .cargo_metadata(false)
        .cargo_warnings(false)
        .warnings(false)
        .target(&triple)
        .host(&triple)
        .opt_level(2)
        .out_dir(&obj_dir)
        .include(&src)
        .files(&sources);

    if cfg!(windows) {
        build.define("LUA_USE_WINDOWS", None);
    } else {
        build.define("LUA_USE_POSIX", None);
    }

    let objects = build
        .try_compile_intermediates()
        .context("compiling the PUC-Lua sources")?;
    let tool = build.try_get_compiler().context("locating a C compiler")?;
    link(&tool, &objects, &exe)?;

    println!("built {}", exe.display());
    Ok(())
}

/// Every `src/*.c` except `luac.c`.
///
/// Including `luac.c` alongside `lua.c` gives the link two `main` symbols and fails.
fn lua_sources(src: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(src).with_context(|| format!("cannot read {}", src.display()))? {
        let path = entry?.path();
        let is_c = path.extension().is_some_and(|e| e == "c");
        let is_luac = path.file_name().is_some_and(|n| n == "luac.c");
        if is_c && !is_luac {
            files.push(path)
        }
    }

    if files.is_empty() {
        bail!(
            "no C sources under {} - is vendor/ populated?",
            src.display()
        );
    }
    files.sort();

    Ok(files)
}

fn is_up_to_date(exe: &Path, src: &Path) -> Result<bool> {
    let Ok(built_at) = exe.metadata().and_then(|m| m.modified()) else {
        return Ok(false);
    };

    let mut newest = SystemTime::UNIX_EPOCH;
    for entry in std::fs::read_dir(src)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "c" || e == "h") {
            newest = newest.max(path.metadata()?.modified()?);
        }
    }

    Ok(built_at > newest)
}

/// Link the object files into an executable.
///
/// MSVC goes through `link.exe` rather than `cl.exe`.
fn link(tool: &cc::Tool, objects: &[PathBuf], exe: &Path) -> Result<()> {
    let mut cmd = if tool.is_like_msvc() {
        let mut cmd = Command::new("link.exe");
        cmd.envs(tool.env().iter().cloned());
        cmd.arg("-nologo").arg(format!("-out:{}", exe.display()));
        cmd
    } else {
        let mut cmd = tool.to_command();
        cmd.arg("-o").arg(exe).arg("-lm");
        cmd
    };
    cmd.args(objects);

    let status = cmd
        .status()
        .with_context(|| format!("cannot run {cmd:?}"))?;
    if !status.success() {
        bail!("linking the reference interpreter failed: {status}");
    }

    Ok(())
}

/// The host target triple, from `rustc -vV`.
fn host_triple() -> Result<String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .output()
        .context("cannot run `rustc -vV`")?;

    let stdout = String::from_utf8(output.stdout).context("`rustc -vV` output is not UTF-8")?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("no `host:` line in `rustc -vV` output")
}
