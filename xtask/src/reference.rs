//! Building the PUC-Lua reference binaries with the `cc` crate rather tan Lua's makefile.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

use crate::repo_root;

/// Which of PUC-Lua's two drivers to link.
#[derive(Clone, Copy)]
pub(crate) enum Driver {
    Lua,
    Luac,
}

impl Driver {
    fn stem(self) -> &'static str {
        match self {
            Driver::Lua => "lua",
            Driver::Luac => "luac",
        }
    }

    /// The driver source that must be left out of this build.
    fn excluded(self) -> &'static str {
        match self {
            Driver::Lua => "luac.c",
            Driver::Luac => "lua.c",
        }
    }
}

/// Build PUC-Lua to produce `target/reference/<driver>[.exe]`
pub(crate) fn build(driver: Driver) -> Result<()> {
    let root = repo_root();
    let src = root.join("vendor").join("puc-lua").join("src");
    let out_dir = root.join("target").join("reference");
    // Separate object directories: a shared one would mix lua.o into the luac link.
    let obj_dir = out_dir.join("obj").join(driver.stem());
    let exe = out_dir.join(if cfg!(windows) {
        format!("{}.exe", driver.stem())
    } else {
        driver.stem().to_owned()
    });

    if is_up_to_date(&exe, &src)? {
        println!("reference up to date: {}", exe.display());
        return Ok(());
    }

    let sources = lua_sources(&src, driver)?;
    fs::create_dir_all(&obj_dir).with_context(|| format!("cannot create {}", obj_dir.display()))?;

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

/// Every `src/*.c` except the driver this build does not want.
fn lua_sources(src: &Path, driver: Driver) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(src).with_context(|| format!("cannot read {}", src.display()))? {
        let path = entry?.path();
        let is_c = path.extension().is_some_and(|e| e == "c");
        let is_excluded = path.file_name().is_some_and(|n| n == driver.excluded());
        if is_c && !is_excluded {
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
    for entry in fs::read_dir(src)? {
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
