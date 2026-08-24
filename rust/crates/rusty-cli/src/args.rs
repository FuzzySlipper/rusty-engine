use std::path::PathBuf;

use crate::report::Diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Init {
        target: PathBuf,
        product_id: Option<String>,
    },
    Check {
        start: PathBuf,
    },
    Doctor {
        start: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) command: Command,
    pub(crate) format: OutputFormat,
}

impl Invocation {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, Diagnostic> {
        let arguments = arguments.into_iter().collect::<Vec<_>>();
        let command = arguments.first().map(String::as_str).ok_or_else(usage)?;
        let mut format = OutputFormat::Human;
        let mut path = None;
        let mut product_id = None;
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--json" => format = OutputFormat::Json,
                "--path" => {
                    index += 1;
                    let value = arguments.get(index).ok_or_else(usage)?;
                    path = Some(PathBuf::from(value));
                }
                "--id" if command == "init" => {
                    index += 1;
                    product_id = Some(arguments.get(index).ok_or_else(usage)?.clone());
                }
                _ => return Err(usage()),
            }
            index += 1;
        }
        let path = path.unwrap_or_else(|| PathBuf::from("."));
        let command = match command {
            "init" => Command::Init {
                target: path,
                product_id,
            },
            "check" => Command::Check { start: path },
            "doctor" => Command::Doctor { start: path },
            _ => return Err(usage()),
        };
        Ok(Self { command, format })
    }
}

fn usage() -> Diagnostic {
    Diagnostic::error(
        "RUSTY_USAGE",
        "$",
        "usage: rusty <init|check|doctor> [--path <path>] [--json]; init also accepts --id <product-id>",
    )
}
