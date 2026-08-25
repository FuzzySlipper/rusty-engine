//! Thin process composition for the bounded Product Model CLI.

#![forbid(unsafe_code)]

mod args;
mod check;
mod commands;
mod desktop;
mod init;
mod inspect;
mod kernel_probe;
mod package;
mod report;
mod workflow;

use std::{
    env,
    path::{Path, PathBuf},
};

use args::{Command, Invocation, OutputFormat};
use report::{emit, Report};

pub(crate) const EXIT_USAGE: i32 = 2;
pub(crate) const EXIT_ROOT: i32 = 3;
pub(crate) const EXIT_CONFORMANCE: i32 = 4;

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
    let format = invocation.format;
    match invocation.command {
        Command::Init { target, product_id } => init::init(resolve_from(cwd, &target), product_id),
        Command::Check { start } => commands::check(resolve_from(cwd, &start)),
        Command::Doctor { start } => commands::doctor(resolve_from(cwd, &start)),
        Command::Dev { start, port } => commands::dev(resolve_from(cwd, &start), port, format),
        Command::Test { start, wrapper } => commands::test(resolve_from(cwd, &start), wrapper),
        Command::Inspect { start, subject } => {
            commands::inspect(resolve_from(cwd, &start), &subject)
        }
        Command::Build { start } => commands::build(resolve_from(cwd, &start)),
        Command::Package { start, wrapper } => {
            commands::package(resolve_from(cwd, &start), wrapper)
        }
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
