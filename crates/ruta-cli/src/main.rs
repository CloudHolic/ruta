//! Executable `ruta`, a Lua Interpreter.

use std::io::Write;
use std::process::ExitCode;

use ruta_syntax::error::SyntaxError;
use ruta_syntax::line_index::LineIndex;
use ruta_syntax::parser::parse_chunk;

#[cfg(windows)]
const LINE_END: &[u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_END: &[u8] = b"\n";

fn main() -> ExitCode {
    let mut args = std::env::args();
    let progname = args.next().unwrap_or_else(|| "ruta".to_owned());

    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("-p"), Some(path), None) => parse_only(&progname, &path),
        _ => {
            report(format!("{progname}: usage: ruta -p <file>").as_bytes());
            ExitCode::FAILURE
        }
    }
}

/// Parse without executing, the way `luac -p` does.
fn parse_only(progname: &str, path: &str) -> ExitCode {
    let source = match std::fs::read(path) {
        Ok(bytes) => strip_prelude(&bytes),
        Err(error) => {
            report(format!("{progname}: cannot open {path}: {error}").as_bytes());
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = parse_chunk(&source) {
        report_syntax_error(progname, path, &error, &source);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Lua's own library writes through the C runtime, which on Windows turns `\n` into `\r\n`.
fn report(line: &[u8]) {
    let mut out = Vec::with_capacity(line.len() + LINE_END.len());
    out.extend_from_slice(line);
    out.extend_from_slice(LINE_END);

    // Nothing left to report to if stderr itself cannot be written.
    let _ = std::io::stderr().write_all(&out);
}

fn report_syntax_error(progname: &str, path: &str, error: &SyntaxError, source: &[u8]) {
    let lines = LineIndex::new(source);
    let mut line = format!("{progname}: {path}:{}: ", error.line(&lines)).into_bytes();
    line.extend_from_slice(&error.message(&lines));

    report(&line);
}

fn strip_prelude(source: &[u8]) -> Vec<u8> {
    let body = if source.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &source[3..]
    } else {
        source
    };

    if body.first() != Some(&b'#') {
        return body.to_vec();
    }

    let rest = match body.iter().position(|byte| *byte == b'\n') {
        Some(at) => &body[at + 1..],
        None => &[][..],
    };

    let mut out = Vec::with_capacity(rest.len() + 1);
    out.push(b'\n');
    out.extend_from_slice(rest);
    out
}
