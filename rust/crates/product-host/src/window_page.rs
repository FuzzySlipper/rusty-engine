//! The desktop window's UI page as Engine live-debug commands, so an agent
//! evaluates JavaScript and reads the DOM the same way under either UI
//! engine, with no debugging port. The host answers them outside the
//! runtime's serialization: a page script it awaits may call the host.
use std::time::Duration;

pub(crate) const EVAL_COMMAND: &str = "engine.ui.eval";
pub(crate) const DOM_COMMAND: &str = "engine.ui.dom";

/// How long a command waits for the page; a page script that never settles
/// answers with this.
const PAGE_ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// The desktop window's UI page, which the runtime gives the host in window
/// output.
pub trait ProductHostWindowPage: Send + Sync {
    /// Evaluate `source` in the page and wait up to `timeout` for its
    /// answer: the page's JSON (`{"value": ...}` or `{"error": ...}`), or why
    /// there is none.
    fn evaluate(&self, source: String, timeout: Duration) -> Result<String, String>;
    /// The web engine drawing the page, once it opened.
    fn engine(&self) -> Option<String>;
}

#[derive(serde::Serialize)]
struct Answer {
    /// The web engine that ran the script.
    ui: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// When the page had laid out what the finished script changed; a
    /// repaint shown after it shows the change.
    #[serde(rename = "renderedAtUnixMs")]
    rendered_at_unix_ms: Option<f64>,
}

#[derive(serde::Deserialize)]
struct PageAnswer {
    value: Option<serde_json::Value>,
    error: Option<String>,
    #[serde(rename = "renderedAtUnixMs")]
    rendered_at_unix_ms: Option<f64>,
}

/// Answers `engine.ui.eval <expression>`, where the expression is the rest
/// of the line, and `engine.ui.dom [selector]`, with whether it succeeded and
/// the reply; none for any other command.
pub(crate) fn execute(
    page: Option<&dyn ProductHostWindowPage>,
    command: &str,
) -> Option<(bool, String)> {
    let command = command.trim();
    let (name, rest) = command
        .split_once(char::is_whitespace)
        .unwrap_or((command, ""));
    let rest = rest.trim();
    let source = match name {
        EVAL_COMMAND if rest.is_empty() => {
            return Some((false, format!("Usage: {EVAL_COMMAND} <expression>")))
        }
        EVAL_COMMAND => rest.to_owned(),
        DOM_COMMAND if rest.is_empty() => "document.documentElement.outerHTML".to_owned(),
        // A selector's JSON string is a valid JavaScript string literal.
        DOM_COMMAND => format!(
            "(() => {{ const element = document.querySelector({}); \
             if (!element) throw new Error('no element matches ' + {0}); \
             return element.outerHTML; }})()",
            serde_json::Value::String(rest.to_owned())
        ),
        _ => return None,
    };
    let Some(page) = page else {
        return Some((
            false,
            format!(
                "{name} reads the desktop window's UI page; this runtime streams its frames, \
                 so its page runs in the viewer's browser"
            ),
        ));
    };
    let answer = page.evaluate(source, PAGE_ANSWER_TIMEOUT).and_then(|json| {
        serde_json::from_str::<PageAnswer>(&json)
            .map_err(|error| format!("the page's answer is not JSON: {error}"))
    });
    let rendered_at_unix_ms = answer
        .as_ref()
        .ok()
        .and_then(|answer| answer.rendered_at_unix_ms);
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
        ui: page.engine(),
        value,
        error,
        rendered_at_unix_ms,
    };
    Some(match serde_json::to_string_pretty(&answer) {
        Ok(message) => (succeeded, message),
        Err(error) => (false, error.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A page that answers with `answer` and keeps the last script.
    struct Page {
        answer: Result<String, String>,
        script: Mutex<String>,
    }

    impl ProductHostWindowPage for Page {
        fn evaluate(&self, source: String, _: Duration) -> Result<String, String> {
            *self.script.lock().unwrap() = source;
            self.answer.clone()
        }

        fn engine(&self) -> Option<String> {
            Some("servo 0.7.0".to_owned())
        }
    }

    fn page(answer: Result<&str, &str>) -> Page {
        Page {
            answer: answer.map(str::to_owned).map_err(str::to_owned),
            script: Mutex::default(),
        }
    }

    #[test]
    fn eval_answers_the_value_or_what_was_thrown_with_the_engine() {
        let value = page(Ok(r#"{"value":{"n":3},"renderedAtUnixMs":1792996168000}"#));
        let (succeeded, message) = execute(Some(&value), "engine.ui.eval 1 + 2").unwrap();
        assert!(succeeded);
        assert_eq!(*value.script.lock().unwrap(), "1 + 2");
        let message: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(message["ui"], "servo 0.7.0");
        assert_eq!(message["value"]["n"], 3);
        assert_eq!(message["renderedAtUnixMs"], 1792996168000.0);

        let thrown = page(Ok(r#"{"error":"Error: boom"}"#));
        let (succeeded, message) = execute(Some(&thrown), "engine.ui.eval boom()").unwrap();
        assert!(!succeeded);
        assert!(message.contains("Error: boom"));

        let silent = page(Err("the window's UI page did not answer within 5 s"));
        assert!(
            !execute(Some(&silent), "engine.ui.eval new Promise(() => {})")
                .unwrap()
                .0
        );
    }

    #[test]
    fn dom_quotes_its_selector_and_other_commands_pass_through() {
        let dom = page(Ok(r#"{"value":"<p></p>"}"#));
        assert!(
            execute(Some(&dom), r#"engine.ui.dom a[title="x'y"]"#)
                .unwrap()
                .0
        );
        assert!(dom
            .script
            .lock()
            .unwrap()
            .contains(r#"document.querySelector("a[title=\"x'y\"]")"#));
        assert!(execute(Some(&dom), "engine.ui.dom").unwrap().0);
        assert_eq!(
            *dom.script.lock().unwrap(),
            "document.documentElement.outerHTML"
        );
        assert!(execute(Some(&dom), "engine.renderer").is_none());
        assert!(!execute(Some(&dom), "engine.ui.eval").unwrap().0);
        let (succeeded, message) = execute(None, "engine.ui.eval 1").unwrap();
        assert!(!succeeded && message.contains("streams its frames"));
    }
}
