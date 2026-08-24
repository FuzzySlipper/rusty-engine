//! Thin process composition for the bounded Product Model CLI.

#![forbid(unsafe_code)]

mod args;
mod check;
mod init;
mod report;

use std::{
    env,
    path::{Path, PathBuf},
};

use args::{Command, Invocation, OutputFormat};
use report::{emit, Report};

pub(crate) const EXIT_USAGE: i32 = 2;
pub(crate) const EXIT_ROOT: i32 = 3;
pub(crate) const EXIT_CONFORMANCE: i32 = 4;
pub(crate) const EXIT_INCOMPLETE: i32 = 5;

pub(crate) struct Execution {
    pub(crate) report: Report,
    pub(crate) exit_code: i32,
}

fn main() {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("rusty: unable to determine the current directory: {error}");
            std::process::exit(EXIT_ROOT);
        }
    };
    let invocation = match Invocation::parse(env::args().skip(1)) {
        Ok(invocation) => invocation,
        Err(diagnostic) => {
            emit(&Report::failure("error", diagnostic), OutputFormat::Human);
            std::process::exit(EXIT_USAGE);
        }
    };
    let format = invocation.format;
    let result = execute(invocation, &cwd);
    emit(&result.report, format);
    std::process::exit(result.exit_code);
}

pub(crate) fn execute(invocation: Invocation, cwd: &Path) -> Execution {
    match invocation.command {
        Command::Init { target, product_id } => init::init(resolve_from(cwd, &target), product_id),
        Command::Check { start } => check::check(resolve_from(cwd, &start)),
        Command::Doctor { start } => check::doctor(resolve_from(cwd, &start)),
    }
}

fn resolve_from(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests;
