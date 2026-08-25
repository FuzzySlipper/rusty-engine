use serde::Serialize;

use crate::args::OutputFormat;

/// At most twelve diagnostics are retained. With two independently bounded
/// 1,024-byte strings per diagnostic and worst-case JSON escaping, serialized
/// JSON remains below this conservative aggregate ceiling without ever
/// truncating JSON into an invalid document.
pub(crate) const MAX_DIAGNOSTICS: usize = 12;
pub(crate) const MAX_FACTS: usize = 256;
pub(crate) const MAX_SERIALIZED_REPORT_BYTES: usize = 160 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Diagnostic {
    pub(crate) level: &'static str,
    pub(crate) code: &'static str,
    pub(crate) path: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Fact {
    pub(crate) path: String,
    pub(crate) value: String,
}

impl Fact {
    pub(crate) fn new(path: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            path: bounded(path.into()),
            value: bounded(value.into()),
        }
    }
}

impl Diagnostic {
    pub(crate) fn error(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new("error", code, path, message)
    }

    pub(crate) fn note(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new("note", code, path, message)
    }

    fn new(
        level: &'static str,
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            level,
            code,
            path: bounded(path.into()),
            message: bounded(message.into()),
        }
    }
}

fn bounded(value: String) -> String {
    if value.len() <= MAX_DIAGNOSTIC_BYTES {
        return value;
    }
    const ELLIPSIS: &str = "…";
    let mut end = MAX_DIAGNOSTIC_BYTES - ELLIPSIS.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ELLIPSIS}", &value[..end])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    pub(crate) status: &'static str,
    pub(crate) diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) facts: Vec<Fact>,
}

impl Report {
    pub(crate) fn success() -> Self {
        Self {
            status: "ok",
            diagnostics: Vec::new(),
            facts: Vec::new(),
        }
    }

    pub(crate) fn incomplete(mut diagnostics: Vec<Diagnostic>) -> Self {
        normalize_diagnostics(&mut diagnostics);
        Self {
            status: if diagnostics.iter().any(|item| item.level == "error") {
                "error"
            } else {
                "incomplete"
            },
            diagnostics,
            facts: Vec::new(),
        }
    }

    pub(crate) fn checked(mut diagnostics: Vec<Diagnostic>) -> Self {
        normalize_diagnostics(&mut diagnostics);
        Self {
            status: if diagnostics.iter().any(|item| item.level == "error") {
                "error"
            } else {
                "ok"
            },
            diagnostics,
            facts: Vec::new(),
        }
    }

    pub(crate) fn failure(status: &'static str, diagnostic: Diagnostic) -> Self {
        Self {
            status,
            diagnostics: vec![diagnostic],
            facts: Vec::new(),
        }
    }

    pub(crate) fn with_facts(mut self, mut facts: Vec<Fact>) -> Self {
        facts.sort_by(|left, right| (&left.path, &left.value).cmp(&(&right.path, &right.value)));
        facts.dedup();
        if facts.len() > MAX_FACTS {
            facts.truncate(MAX_FACTS);
            self.diagnostics.push(Diagnostic::error(
                "RUSTY_FACT_LIMIT",
                "$",
                format!("report facts are limited to {MAX_FACTS} entries"),
            ));
            self.status = "error";
        }
        self.facts = facts;
        self
    }

    pub(crate) fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|item| item.level == "error")
    }
}

fn normalize_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.sort_by(|left, right| {
        (&left.path, left.code, &left.message).cmp(&(&right.path, right.code, &right.message))
    });
    if diagnostics.len() > MAX_DIAGNOSTICS {
        diagnostics.truncate(MAX_DIAGNOSTICS - 1);
        diagnostics.push(Diagnostic::error(
            "RUSTY_DIAGNOSTIC_LIMIT",
            "$",
            format!("diagnostics are limited to {MAX_DIAGNOSTICS} entries"),
        ));
    }
}

pub(crate) fn emit(report: &Report, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            let encoded = serde_json::to_string(report).expect("the stable report is serializable");
            if encoded.len() < MAX_SERIALIZED_REPORT_BYTES {
                println!("{encoded}");
            } else {
                let fallback = Report::failure(
                    "error",
                    Diagnostic::error(
                        "RUSTY_REPORT_BYTES_EXCEEDED",
                        "$",
                        format!("serialized diagnostics exceed the {MAX_SERIALIZED_REPORT_BYTES}-byte report limit"),
                    ),
                );
                println!(
                    "{}",
                    serde_json::to_string(&fallback).expect("the fallback report is serializable")
                );
            }
        }
        OutputFormat::Human => {
            println!("status: {}", report.status);
            for diagnostic in &report.diagnostics {
                println!(
                    "{} {} {}: {}",
                    diagnostic.level.to_ascii_uppercase(),
                    diagnostic.code,
                    diagnostic.path,
                    diagnostic.message
                );
            }
            for fact in &report.facts {
                println!("{}: {}", fact.path, fact.value);
            }
        }
    }
}
