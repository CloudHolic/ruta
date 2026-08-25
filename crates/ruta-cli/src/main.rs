//! Executable `ruta`, a Lua Interpreter.

use std::io::Write;
use std::process::ExitCode;

use ruta_syntax::lexer::{LexError, Lexer, LineIndex};
use ruta_syntax::token::TokenKind;

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
    let source = match std::fs::read(path) {
        Ok(bytes) => strip_prelude(&bytes),
        Err(error) => {
            eprintln!("{progname}: cannot open {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut lexer = Lexer::new(&source);
    loop {
        match lexer.next_token() {
            Ok(token) => {
                if matches!(token.kind, TokenKind::Eof) {
                    break;
                }
            }
            Err(error) => {
                report_lex_error(progname, path, &error, &source);
                return ExitCode::FAILURE;
            }
        }
    }

    eprintln!("{progname}: {path}: parsing is not implemented");
    ExitCode::FAILURE
}

fn report_lex_error(progname: &str, path: &str, error: &LexError, source: &[u8]) {
    let lines = LineIndex::new(source);
    let mut out = format!("{progname}: {path}:{}: ", error.line(&lines)).into_bytes();
    out.extend_from_slice(&error.message(source));
    out.push(b'\n');

    // Nothing left to report to if stderr itself cannot be written.
    let _ = std::io::stderr().write_all(&out);
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
		None => a[][..],
	};

	let mut out = Vec::with_capacity(rest.len() + 1);
	out.push(b'\n');
	out.extend_from_slice(rest);
	out
}
