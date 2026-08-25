//! Executable `ruta`, a Lua Interpreter.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args();
    let progname = args.next().unwrap_or_else(|| "ruta".to_owned());

    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("-p"), Some(path), None) => parse_only(&progname, &path),
        _ => {
            eprintln!("{progname}: usage: ruta -p <file>");
            ExitCode::FAILURE
        }
    }
}

///Parse without executing, the way `luac -p` does.
fn parse_only(progname: &str, path: &str) -> ExitCode {
    eprintln!("{progname}: {path}: parsing is not implemented");
    ExitCode::FAILURE
}
