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
    Dev {
        start: PathBuf,
        port: u16,
    },
    Test {
        start: PathBuf,
    },
    Inspect {
        start: PathBuf,
        subject: String,
    },
    Build {
        start: PathBuf,
    },
    Package {
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
        let mut port = 0_u16;
        let mut subject = None;
        let mut index = 1;
        if command == "inspect" {
            subject = arguments.get(index).cloned();
            if subject.is_none() {
                return Err(usage());
            }
            index += 1;
        }
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
                "--port" if command == "dev" => {
                    index += 1;
                    port = arguments
                        .get(index)
                        .ok_or_else(usage)?
                        .parse()
                        .map_err(|_| usage())?;
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
            "dev" => Command::Dev { start: path, port },
            "test" => Command::Test { start: path },
            "inspect" => Command::Inspect {
                start: path,
                subject: subject.expect("inspect subject was required above"),
            },
            "build" => Command::Build { start: path },
            "package" => Command::Package { start: path },
            _ => return Err(usage()),
        };
        Ok(Self { command, format })
    }
}

fn usage() -> Diagnostic {
    Diagnostic::error(
        "RUSTY_USAGE",
        "$",
        "usage: rusty <init|dev|check|test|inspect|build|package|doctor> [subject] [--path <path>] [--json]; init also accepts --id <product-id>, dev accepts --port <u16>",
    )
}

#[cfg(test)]
mod tests {
    use super::{Command, Invocation, OutputFormat};

    #[test]
    fn product_workflow_commands_parse_only_their_closed_options() {
        let dev = Invocation::parse([
            "dev".to_owned(),
            "--path".to_owned(),
            "product".to_owned(),
            "--port".to_owned(),
            "4321".to_owned(),
            "--json".to_owned(),
        ])
        .expect("dev invocation");
        assert_eq!(dev.format, OutputFormat::Json);
        assert!(matches!(dev.command, Command::Dev { port: 4321, .. }));

        let inspect = Invocation::parse([
            "inspect".to_owned(),
            "capability-bindings".to_owned(),
            "--path".to_owned(),
            "product".to_owned(),
        ])
        .expect("inspect invocation");
        assert!(matches!(
            inspect.command,
            Command::Inspect { ref subject, .. } if subject == "capability-bindings"
        ));

        assert!(
            Invocation::parse(["build".to_owned(), "--port".to_owned(), "1".to_owned()]).is_err()
        );
        assert!(Invocation::parse(["inspect".to_owned()]).is_err());
    }
}
