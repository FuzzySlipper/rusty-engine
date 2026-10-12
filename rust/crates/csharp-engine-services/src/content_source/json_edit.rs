//! JSON pointer edits applied to JSON text, keeping every byte outside the
//! edited values: key order, indentation, number spelling and untouched lines.

use serde_json::Value;

/// One edit, addressed by an RFC 6901 JSON pointer.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonEdit {
    /// Replaces the value the pointer names.
    Replace { pointer: String, value: Value },
    /// Adds an object member (replacing an existing one) or inserts an array
    /// element before an index, or at the end for `-`.
    Add { pointer: String, value: Value },
    /// Removes an object member or an array element.
    Remove { pointer: String },
}

/// Why an edit could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonEditError {
    /// The text is not JSON.
    Text(String),
    /// The pointer does not name a value the edit can apply to.
    Pointer(String),
}

/// `text` with every edit applied in order.
pub fn apply(text: &str, edits: &[JsonEdit]) -> Result<String, JsonEditError> {
    let mut text = text.to_owned();
    for edit in edits {
        text = apply_one(&text, edit)?;
    }
    serde_json::from_str::<Value>(&text)
        .map_err(|error| JsonEditError::Text(format!("the edited text is not JSON: {error}")))?;
    Ok(text)
}

fn apply_one(text: &str, edit: &JsonEdit) -> Result<String, JsonEditError> {
    let root = Parser::parse(text)?;
    let (pointer, tokens) = match edit {
        JsonEdit::Replace { pointer, .. }
        | JsonEdit::Add { pointer, .. }
        | JsonEdit::Remove { pointer } => (pointer, tokens(pointer)?),
    };
    let missing = || JsonEditError::Pointer(format!("`{pointer}` does not name a value"));
    let splice = |start: usize, end: usize, with: &str| {
        let mut edited = String::with_capacity(text.len() + with.len());
        edited.push_str(&text[..start]);
        edited.push_str(with);
        edited.push_str(&text[end..]);
        edited
    };
    if let JsonEdit::Replace { value, .. } = edit {
        let node = root.find(&tokens).ok_or_else(missing)?;
        return Ok(splice(node.start, node.end, &compact(value)));
    }
    let (last, parent_tokens) = tokens.split_last().ok_or_else(|| {
        JsonEditError::Pointer("the whole document cannot be added or removed".into())
    })?;
    let parent = root.find(parent_tokens).ok_or_else(missing)?;
    match (&parent.kind, edit) {
        (Kind::Object(members), JsonEdit::Add { value, .. }) => {
            if let Some(member) = members.iter().find(|member| &member.key == last) {
                return Ok(splice(
                    member.value.start,
                    member.value.end,
                    &compact(value),
                ));
            }
            let key = compact(&Value::String(last.clone()));
            Ok(match members.last() {
                None => splice(
                    parent.start + 1,
                    parent.end - 1,
                    &format!("{key}: {}", compact(value)),
                ),
                Some(previous) => {
                    let separator =
                        separator(text, parent, members.iter().map(|m| (m.start, m.value.end)));
                    let colon = &text[previous.key_end..previous.value.start];
                    let at = previous.value.end;
                    splice(
                        at,
                        at,
                        &format!("{separator}{key}{colon}{}", compact(value)),
                    )
                }
            })
        }
        (Kind::Array(elements), JsonEdit::Add { value, .. }) => {
            let index = if last == "-" {
                elements.len()
            } else {
                array_index(last)
                    .filter(|index| *index <= elements.len())
                    .ok_or_else(missing)?
            };
            let value = compact(value);
            let separator = separator(text, parent, elements.iter().map(|e| (e.start, e.end)));
            Ok(match (elements.get(index), elements.last()) {
                (_, None) => splice(parent.start + 1, parent.end - 1, &value),
                (Some(next), _) => splice(next.start, next.start, &format!("{value}{separator}")),
                (None, Some(previous)) => {
                    splice(previous.end, previous.end, &format!("{separator}{value}"))
                }
            })
        }
        (Kind::Object(members), JsonEdit::Remove { .. }) => {
            let index = members
                .iter()
                .position(|member| &member.key == last)
                .ok_or_else(missing)?;
            let spans: Vec<_> = members.iter().map(|m| (m.start, m.value.end)).collect();
            let (start, end) = removal(parent, &spans, index);
            Ok(splice(start, end, ""))
        }
        (Kind::Array(elements), JsonEdit::Remove { .. }) => {
            let index = array_index(last)
                .filter(|index| *index < elements.len())
                .ok_or_else(missing)?;
            let spans: Vec<_> = elements.iter().map(|e| (e.start, e.end)).collect();
            let (start, end) = removal(parent, &spans, index);
            Ok(splice(start, end, ""))
        }
        _ => Err(missing()),
    }
}

/// The text between two neighbouring items (`,` and the indentation), or
/// `,` and the container's leading whitespace when it holds one item.
fn separator(
    text: &str,
    container: &Node,
    mut items: impl Iterator<Item = (usize, usize)>,
) -> String {
    let Some((first_start, first_end)) = items.next() else {
        return ",".to_owned();
    };
    match items.next() {
        Some((second_start, _)) => text[first_end..second_start].to_owned(),
        None => format!(",{}", &text[container.start + 1..first_start]),
    }
}

/// The span to delete for item `index`: through the next item's start, back
/// to the previous item's end for the last one, or the whole inside of a
/// container holding only it.
fn removal(container: &Node, spans: &[(usize, usize)], index: usize) -> (usize, usize) {
    let (start, end) = spans[index];
    if let Some((next_start, _)) = spans.get(index + 1) {
        (start, *next_start)
    } else if index > 0 {
        (spans[index - 1].1, end)
    } else {
        (container.start + 1, container.end - 1)
    }
}

fn compact(value: &Value) -> String {
    value.to_string()
}

fn array_index(token: &str) -> Option<usize> {
    if token.len() > 1 && token.starts_with('0') {
        return None;
    }
    token.parse().ok()
}

/// A pointer's reference tokens, unescaped (`~1` is `/`, `~0` is `~`).
fn tokens(pointer: &str) -> Result<Vec<String>, JsonEditError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    let rest = pointer.strip_prefix('/').ok_or_else(|| {
        JsonEditError::Pointer(format!(
            "`{pointer}` is not a JSON pointer: it must start with `/`"
        ))
    })?;
    Ok(rest
        .split('/')
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect())
}

/// A JSON value and where its text lies.
struct Node {
    start: usize,
    end: usize,
    kind: Kind,
}

enum Kind {
    Scalar,
    Object(Vec<Member>),
    Array(Vec<Node>),
}

struct Member {
    key: String,
    /// The start of the key's opening quote.
    start: usize,
    /// Just past the key's closing quote.
    key_end: usize,
    value: Node,
}

impl Node {
    fn find(&self, tokens: &[String]) -> Option<&Node> {
        let Some((first, rest)) = tokens.split_first() else {
            return Some(self);
        };
        let child = match &self.kind {
            Kind::Object(members) => &members.iter().find(|member| &member.key == first)?.value,
            Kind::Array(elements) => elements.get(array_index(first)?)?,
            Kind::Scalar => return None,
        };
        child.find(rest)
    }
}

/// Finds value spans; `apply` checks the edited text with serde_json, so this
/// only needs to follow the structure.
struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn parse(text: &'a str) -> Result<Node, JsonEditError> {
        let mut parser = Self { text, at: 0 };
        let node = parser.value()?;
        parser.whitespace();
        if parser.at != text.len() {
            return Err(parser.error("text after the document"));
        }
        Ok(node)
    }

    fn error(&self, what: &str) -> JsonEditError {
        JsonEditError::Text(format!("{what} at byte {}", self.at))
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonEditError> {
        self.whitespace();
        if self.peek() != Some(byte) {
            return Err(self.error(&format!("expected `{}`", byte as char)));
        }
        self.at += 1;
        Ok(())
    }

    fn value(&mut self) -> Result<Node, JsonEditError> {
        self.whitespace();
        let start = self.at;
        let kind = match self.peek() {
            Some(b'{') => Kind::Object(self.object()?),
            Some(b'[') => Kind::Array(self.array()?),
            Some(b'"') => {
                self.string()?;
                Kind::Scalar
            }
            Some(_) => {
                while matches!(self.peek(), Some(byte) if !b",]} \t\n\r".contains(&byte)) {
                    self.at += 1;
                }
                if self.at == start {
                    return Err(self.error("expected a value"));
                }
                Kind::Scalar
            }
            None => return Err(self.error("expected a value")),
        };
        Ok(Node {
            start,
            end: self.at,
            kind,
        })
    }

    fn object(&mut self) -> Result<Vec<Member>, JsonEditError> {
        self.at += 1;
        let mut members = Vec::new();
        self.whitespace();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(members);
        }
        loop {
            self.whitespace();
            let start = self.at;
            let key = self.string()?;
            let key_end = self.at;
            self.expect(b':')?;
            let value = self.value()?;
            members.push(Member {
                key,
                start,
                key_end,
                value,
            });
            self.whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(members);
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self) -> Result<Vec<Node>, JsonEditError> {
        self.at += 1;
        let mut elements = Vec::new();
        self.whitespace();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(elements);
        }
        loop {
            elements.push(self.value()?);
            self.whitespace();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(elements);
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
    }

    /// A string, decoded.
    fn string(&mut self) -> Result<String, JsonEditError> {
        if self.peek() != Some(b'"') {
            return Err(self.error("expected a string"));
        }
        let start = self.at;
        self.at += 1;
        loop {
            match self.peek() {
                Some(b'"') => break,
                Some(b'\\') => self.at += 2,
                Some(_) => self.at += 1,
                None => return Err(self.error("unterminated string")),
            }
        }
        self.at += 1;
        serde_json::from_str(&self.text[start..self.at]).map_err(|_| self.error("invalid string"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TEXT: &str = r#"{
  "name": "pistol",
  "stance": { "x": 0.250, "y": -1e-2 },
  "keys": [
    1,
    2
  ],
  "a/b": 1, "t~": [true]
}
"#;

    fn replace(pointer: &str, value: Value) -> JsonEdit {
        JsonEdit::Replace {
            pointer: pointer.into(),
            value,
        }
    }

    fn add(pointer: &str, value: Value) -> JsonEdit {
        JsonEdit::Add {
            pointer: pointer.into(),
            value,
        }
    }

    fn remove(pointer: &str) -> JsonEdit {
        JsonEdit::Remove {
            pointer: pointer.into(),
        }
    }

    #[test]
    fn a_replaced_number_changes_only_its_own_text() {
        let edited = apply(TEXT, &[replace("/stance/x", json!(0.5))]).unwrap();
        assert_eq!(edited, TEXT.replace("0.250", "0.5"));
        let edited = apply(TEXT, &[replace("/keys/1", json!({"at": 3}))]).unwrap();
        assert_eq!(edited, TEXT.replace("    2\n", "    {\"at\":3}\n"));
        let edited = apply(
            TEXT,
            &[replace("/a~1b", json!(2)), replace("/t~0/0", json!(false))],
        )
        .unwrap();
        assert_eq!(
            edited,
            TEXT.replace("\"a/b\": 1", "\"a/b\": 2")
                .replace("[true]", "[false]")
        );
        assert_eq!(apply("[1]", &[replace("", json!(2))]).unwrap(), "2");
    }

    #[test]
    fn adds_follow_their_neighbours_layout() {
        let edited = apply(TEXT, &[add("/keys/-", json!(3))]).unwrap();
        assert_eq!(edited, TEXT.replace("    2\n", "    2,\n    3\n"));
        let edited = apply(TEXT, &[add("/keys/0", json!(0))]).unwrap();
        assert_eq!(edited, TEXT.replace("    1,\n", "    0,\n    1,\n"));
        let edited = apply(TEXT, &[add("/stance/z", json!(1))]).unwrap();
        assert_eq!(
            edited,
            TEXT.replace("\"y\": -1e-2", "\"y\": -1e-2, \"z\": 1")
        );
        let edited = apply(TEXT, &[add("/stance/x", json!(9))]).unwrap();
        assert_eq!(edited, TEXT.replace("0.250", "9"));
        assert_eq!(apply("{}", &[add("/a", json!(1))]).unwrap(), r#"{"a": 1}"#);
        assert_eq!(apply("[ ]", &[add("/-", json!(1))]).unwrap(), "[1]");
        assert_eq!(apply("[1]", &[add("/-", json!(2))]).unwrap(), "[1,2]");
    }

    #[test]
    fn removes_take_one_separator_with_them() {
        let edited = apply(TEXT, &[remove("/keys/0")]).unwrap();
        assert_eq!(edited, TEXT.replace("    1,\n", ""));
        let edited = apply(TEXT, &[remove("/keys/1")]).unwrap();
        assert_eq!(edited, TEXT.replace("1,\n    2\n", "1\n"));
        let edited = apply(TEXT, &[remove("/stance/x")]).unwrap();
        assert_eq!(edited, TEXT.replace("\"x\": 0.250, ", ""));
        assert_eq!(apply("[ 1 ]", &[remove("/0")]).unwrap(), "[]");
    }

    #[test]
    fn a_pointer_that_names_nothing_refuses() {
        for edit in [
            replace("/stance/w", json!(1)),
            replace("/keys/2", json!(1)),
            replace("/keys/01", json!(1)),
            replace("/name/0", json!(1)),
            replace("stance", json!(1)),
            add("/keys/3", json!(1)),
            add("/missing/x", json!(1)),
            remove("/keys/-"),
            remove(""),
        ] {
            assert!(matches!(
                apply(TEXT, &[edit]),
                Err(JsonEditError::Pointer(_))
            ));
        }
        assert!(matches!(
            apply("{\"a\": 1,}", &[replace("/a", json!(2))]),
            Err(JsonEditError::Text(_))
        ));
    }
}
