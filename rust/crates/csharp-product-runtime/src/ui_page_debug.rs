//! The desktop window's UI page as Engine live-debug commands, so an agent
//! evaluates JavaScript and reads the DOM the same way under either UI
//! engine, with no debugging port.
use std::time::Duration;

use super::*;

pub(super) const EVAL_COMMAND: &str = "engine.ui.eval";
pub(super) const DOM_COMMAND: &str = "engine.ui.dom";

/// How long a command waits for the page; a page script that never settles
/// answers with this.
const PAGE_ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(serde::Serialize)]
struct Answer {
    /// The web engine that ran the script.
    ui: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct PageAnswer {
    value: Option<serde_json::Value>,
    error: Option<String>,
}

impl CsharpProductRuntime {
    /// Answers `engine.ui.eval <expression>`, where the expression is the
    /// rest of the line, and `engine.ui.dom [selector]`; none for any other
    /// command.
    pub(super) fn execute_ui_page_debug(&self, command: &str) -> Option<ProductHostDebugResult> {
        let command = command.trim();
        let (name, rest) = command
            .split_once(char::is_whitespace)
            .unwrap_or((command, ""));
        let rest = rest.trim();
        let source = match name {
            EVAL_COMMAND if rest.is_empty() => {
                return Some(ProductHostDebugResult::new(
                    false,
                    format!("Usage: {EVAL_COMMAND} <expression>"),
                ))
            }
            EVAL_COMMAND => rest.to_owned(),
            // A selector's JSON string is a valid JavaScript string literal.
            DOM_COMMAND if rest.is_empty() => "document.documentElement.outerHTML".to_owned(),
            DOM_COMMAND => format!(
                "(() => {{ const element = document.querySelector({}); \
                 if (!element) throw new Error('no element matches ' + {0}); \
                 return element.outerHTML; }})()",
                serde_json::Value::String(rest.to_owned())
            ),
            _ => return None,
        };
        let Some(window) = self
            .frame_output
            .as_ref()
            .and_then(frame_output::FrameOutput::window_timing)
        else {
            return Some(ProductHostDebugResult::new(
                false,
                format!(
                    "{name} reads the desktop window's UI page; this runtime streams its frames, \
                     so its page runs in the viewer's browser"
                ),
            ));
        };
        let answer = window
            .evaluate_in_page(source, PAGE_ANSWER_TIMEOUT)
            .and_then(|json| {
                serde_json::from_str::<PageAnswer>(&json)
                    .map_err(|error| format!("the page's answer is not JSON: {error}"))
            });
        let (succeeded, value, error) = match answer {
            Ok(PageAnswer {
                error: Some(error), ..
            }) => (false, None, Some(error)),
            Ok(PageAnswer { value, .. }) => {
                (true, Some(value.unwrap_or(serde_json::Value::Null)), None)
            }
            Err(error) => (false, None, Some(error)),
        };
        let answer = Answer {
            ui: window.ui_engine(),
            value,
            error,
        };
        Some(match serde_json::to_string_pretty(&answer) {
            Ok(message) => ProductHostDebugResult::new(succeeded, message),
            Err(error) => ProductHostDebugResult::new(false, error.to_string()),
        })
    }
}
