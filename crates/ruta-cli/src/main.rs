//! Executable `ruta`, a Lua Interpreter.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::panic;
use std::process::ExitCode;
use std::thread;

use ruta_compile::{Source, allocate, emit, lower};
use ruta_runtime::vm::Vm;
use ruta_syntax::error::Error;
use ruta_syntax::line_index::LineIndex;
use ruta_syntax::parser::parse_chunk;
use ruta_syntax::scope::resolve;

#[cfg(windows)]
const LINE_END: &[u8] = b"\r\n";
#[cfg(not(windows))]
const LINE_END: &[u8] = b"\n";

const PARSE_STACK: usize = 32 * 1024 * 1024;

/// One chunk to run, in the order the command line gave them.
#[derive(Debug)]
enum Chunk {
    Line(String),
    File(String),
}

fn main() -> ExitCode {
    let mut args = env::args();
    let progname = args.next().unwrap_or_else(|| "ruta".to_owned());
    let args: Vec<String> = args.collect();

    if let [flag, path] = args.as_slice()
        && flag == "-p"
    {
        return parse_only(&progname, path);
    }

    match chunks(&args) {
        Some(chunks) => execute(&progname, &chunks),
        None => {
            report(
                format!("{progname}: usage: ruta [-e stat] ... [script] | ruta -p <file>")
                    .as_bytes(),
            );
            ExitCode::FAILURE
        }
    }
}

/// Parse without executing, the way `luac -p` does.
fn parse_only(progname: &str, path: &str) -> ExitCode {
    let source = match fs::read(path) {
        Ok(bytes) => strip_prelude(&bytes),
        Err(error) => {
            report(format!("{progname}: cannot open {path}: {error}").as_bytes());
            return ExitCode::FAILURE;
        }
    };

    let outcome: io::Result<Result<(), Error>> = thread::scope(|scope| {
        let worker = thread::Builder::new()
            .stack_size(PARSE_STACK)
            .spawn_scoped(scope, || {
                parse_chunk(&source).and_then(|ast| {
                    resolve(&ast).and_then(|bindings| {
                        let mut program = lower(&ast, &bindings)?;
                        allocate(&mut program)?;

                        let lines = LineIndex::new(&source);
                        emit(
                            &program,
                            &Source {
                                lines: &lines,
                                name: path.as_bytes(),
                            },
                        );

                        Ok(())
                    })
                })
            })?;

        Ok(worker
            .join()
            .unwrap_or_else(|payload| panic::resume_unwind(payload)))
    });

    match outcome {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(error)) => {
            report_error(progname, path, &error, &source);
            ExitCode::FAILURE
        }
        Err(error) => {
            report(format!("{progname}: cannot start the parser thread: {error}").as_bytes());
            ExitCode::FAILURE
        }
    }
}

/// Any number of `-e stat`, then at most one script, which comes last.
fn chunks(args: &[String]) -> Option<Vec<Chunk>> {
    let mut out = Vec::new();
    let mut rest = args;

    while let Some((first, tail)) = rest.split_first() {
        match first.as_str() {
            "-e" => {
                let (line, tail) = tail.split_first()?;
                out.push(Chunk::Line(line.clone()));
                rest = tail;
            }
            path if !path.starts_with('-') && tail.is_empty() => {
                out.push(Chunk::File(path.to_owned()));
                rest = tail;
            }
            _ => return None,
        }
    }

    (!out.is_empty()).then_some(out)
}

/// Compiles a file and runs it.
fn execute(progname: &str, chunks: &[Chunk]) -> ExitCode {
    let outcome: io::Result<ExitCode> = thread::scope(|scope| {
        let worker = thread::Builder::new()
            .stack_size(PARSE_STACK)
            .spawn_scoped(scope, || {
                let mut vm = Vm::new();

                for chunk in chunks {
                    let (source, shown, name) = match chunk {
                        Chunk::Line(line) => (
                            line.as_bytes().to_vec(),
                            "(command line)".to_owned(),
                            b"=(command line)".to_vec(),
                        ),
                        Chunk::File(path) => match fs::read(path) {
                            Ok(bytes) => (
                                strip_prelude(&bytes),
                                path.clone(),
                                format!("@{path}").into_bytes(),
                            ),
                            Err(error) => {
                                report(
                                    format!("{progname}: cannot open {path}: {error}").as_bytes(),
                                );
                                return ExitCode::FAILURE;
                            }
                        },
                    };

                    let built = parse_chunk(&source).and_then(|ast| {
                        resolve(&ast).and_then(|bindings| {
                            let mut program = lower(&ast, &bindings)?;
                            allocate(&mut program)?;

                            let lines = LineIndex::new(&source);

                            Ok(emit(
                                &program,
                                &Source {
                                    lines: &lines,
                                    name: &name,
                                },
                            ))
                        })
                    });

                    let prototype = match built {
                        Ok(prototype) => prototype,
                        Err(error) => {
                            report_error(progname, &shown, &error, &source);
                            return ExitCode::FAILURE;
                        }
                    };

                    if let Err(error) = vm.run(prototype) {
                        let mut line = format!("{progname}: ").into_bytes();
                        line.extend_from_slice(&vm.message(&error));
                        report(&line);

                        return ExitCode::FAILURE;
                    }
                }

                ExitCode::SUCCESS
            })?;

        Ok(worker
            .join()
            .unwrap_or_else(|payload| panic::resume_unwind(payload)))
    });

    match outcome {
        Ok(code) => code,
        Err(error) => {
            report(format!("{progname}: cannot start the parser thread: {error}").as_bytes());
            ExitCode::FAILURE
        }
    }
}

/// Lua's own library writes through the C runtime, which on Windows turns `\n` into `\r\n`.
fn report(line: &[u8]) {
    let mut out = Vec::with_capacity(line.len() + LINE_END.len());
    out.extend_from_slice(line);
    out.extend_from_slice(LINE_END);

    // Nothing left to report to if stderr itself cannot be written.
    let _ = io::stderr().write_all(&out);
}

fn report_error(progname: &str, path: &str, error: &Error, source: &[u8]) {
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
