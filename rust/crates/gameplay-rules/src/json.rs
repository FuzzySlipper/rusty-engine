use std::collections::BTreeSet;

use serde_json::{Map, Number, Value};

use crate::{RulePackageError, MAX_SAFE_JSON_INTEGER};

pub(crate) const JSON_MAX_DEPTH: usize = 64;
pub(crate) const JSON_MAX_NODES: usize = 100_000;
pub(crate) const JSON_MAX_STRING_BYTES: usize = 1024 * 1024;

pub(crate) struct JsonBudget {
    nodes: usize,
}

impl JsonBudget {
    pub(crate) const fn new() -> Self {
        Self { nodes: 0 }
    }

    pub(crate) fn nodes(&self) -> usize {
        self.nodes
    }

    pub(crate) fn add_node(&mut self, path: &str) -> Result<(), RulePackageError> {
        self.nodes =
            self.nodes
                .checked_add(1)
                .ok_or_else(|| RulePackageError::ArithmeticOverflow {
                    path: path.to_string(),
                })?;
        if self.nodes > JSON_MAX_NODES {
            return Err(RulePackageError::JsonNodeQuotaExceeded {
                path: path.to_string(),
                actual: self.nodes,
                maximum: JSON_MAX_NODES,
            });
        }
        Ok(())
    }
}

pub(crate) fn parse_strict_json(
    input: &str,
    collection_limits: &[(&str, usize)],
) -> Result<(Value, usize), RulePackageError> {
    let mut parser = Parser {
        input,
        offset: 0,
        budget: JsonBudget::new(),
        collection_limits,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(1, "$")?;
    parser.skip_whitespace();
    if parser.offset != input.len() {
        return Err(parser.malformed("$", "trailing data after the JSON value"));
    }
    Ok((value, parser.budget.nodes()))
}

pub(crate) fn validate_json_value(
    value: &Value,
    depth: usize,
    path: &str,
    budget: &mut JsonBudget,
) -> Result<(), RulePackageError> {
    if depth > JSON_MAX_DEPTH {
        return Err(RulePackageError::JsonDepthExceeded {
            path: path.to_string(),
            actual: depth,
            maximum: JSON_MAX_DEPTH,
        });
    }
    budget.add_node(path)?;
    match value {
        Value::Null | Value::Bool(_) => Ok(()),
        Value::Number(number) => validate_number(number, path),
        Value::String(value) => validate_string(value, path),
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_json_value(value, depth + 1, &pointer_index(path, index), budget)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            for key in keys {
                validate_string(key, &format!("{path}/<key>"))?;
                validate_json_value(&values[key], depth + 1, &pointer_key(path, key), budget)?;
            }
            Ok(())
        }
    }
}

fn validate_number(number: &Number, path: &str) -> Result<(), RulePackageError> {
    if let Some(value) = number.as_i64() {
        if value.unsigned_abs() <= MAX_SAFE_JSON_INTEGER {
            return Ok(());
        }
    } else if let Some(value) = number.as_u64() {
        if value <= MAX_SAFE_JSON_INTEGER {
            return Ok(());
        }
    }
    Err(RulePackageError::JsonIntegerOutOfRange {
        path: path.to_string(),
        value: number.to_string(),
    })
}

pub(crate) fn validate_string(value: &str, path: &str) -> Result<(), RulePackageError> {
    if value.len() > JSON_MAX_STRING_BYTES {
        return Err(RulePackageError::QuotaExceeded {
            path: path.to_string(),
            actual: value.len(),
            maximum: JSON_MAX_STRING_BYTES,
        });
    }
    Ok(())
}

pub(crate) struct BoundedJsonWriter {
    output: Vec<u8>,
    maximum: usize,
}

impl BoundedJsonWriter {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            output: Vec::with_capacity(maximum.min(4_096)),
            maximum,
        }
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.output
    }

    pub(crate) fn push(&mut self, byte: u8, path: &str) -> Result<(), RulePackageError> {
        self.extend(&[byte], path)
    }

    pub(crate) fn extend(&mut self, bytes: &[u8], path: &str) -> Result<(), RulePackageError> {
        let actual = self.output.len().checked_add(bytes.len()).ok_or_else(|| {
            RulePackageError::ArithmeticOverflow {
                path: path.to_string(),
            }
        })?;
        if actual > self.maximum {
            return Err(RulePackageError::ArtifactQuotaExceeded {
                actual,
                maximum: self.maximum,
            });
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn write_value(
        &mut self,
        value: &Value,
        path: &str,
    ) -> Result<(), RulePackageError> {
        match value {
            Value::Null => self.extend(b"null", path),
            Value::Bool(true) => self.extend(b"true", path),
            Value::Bool(false) => self.extend(b"false", path),
            Value::Number(number) => self.extend(number.to_string().as_bytes(), path),
            Value::String(value) => self.write_string(value, path),
            Value::Array(values) => {
                self.push(b'[', path)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.push(b',', path)?;
                    }
                    self.write_value(value, &pointer_index(path, index))?;
                }
                self.push(b']', path)
            }
            Value::Object(values) => {
                self.push(b'{', path)?;
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        self.push(b',', path)?;
                    }
                    self.write_string(key, &format!("{path}/<key>"))?;
                    self.push(b':', path)?;
                    self.write_value(&values[key], &pointer_key(path, key))?;
                }
                self.push(b'}', path)
            }
        }
    }

    pub(crate) fn write_string(&mut self, value: &str, path: &str) -> Result<(), RulePackageError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.push(b'"', path)?;
        for character in value.chars() {
            match character {
                '"' => self.extend(br#"\""#, path)?,
                '\\' => self.extend(br#"\\"#, path)?,
                '\u{0008}' => self.extend(br#"\b"#, path)?,
                '\u{000c}' => self.extend(br#"\f"#, path)?,
                '\n' => self.extend(br#"\n"#, path)?,
                '\r' => self.extend(br#"\r"#, path)?,
                '\t' => self.extend(br#"\t"#, path)?,
                character if character <= '\u{001f}' => {
                    let value = character as u32;
                    self.extend(
                        &[
                            b'\\',
                            b'u',
                            b'0',
                            b'0',
                            HEX[((value >> 4) & 0x0f) as usize],
                            HEX[(value & 0x0f) as usize],
                        ],
                        path,
                    )?;
                }
                character => {
                    let mut encoded = [0; 4];
                    self.extend(character.encode_utf8(&mut encoded).as_bytes(), path)?;
                }
            }
        }
        self.push(b'"', path)
    }
}

pub(crate) fn pointer_key(parent: &str, key: &str) -> String {
    let escaped = key.replace('~', "~0").replace('/', "~1");
    format!("{parent}/{escaped}")
}

pub(crate) fn pointer_index(parent: &str, index: usize) -> String {
    format!("{parent}/{index}")
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
    budget: JsonBudget,
    collection_limits: &'a [(&'a str, usize)],
}

impl Parser<'_> {
    fn parse_value(&mut self, depth: usize, path: &str) -> Result<Value, RulePackageError> {
        if depth > JSON_MAX_DEPTH {
            return Err(RulePackageError::JsonDepthExceeded {
                path: path.to_string(),
                actual: depth,
                maximum: JSON_MAX_DEPTH,
            });
        }
        self.budget.add_node(path)?;
        match self.peek_byte() {
            Some(b'n') => {
                self.expect_literal("null", path)?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.expect_literal("true", path)?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal("false", path)?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => Ok(Value::String(self.parse_string(path)?)),
            Some(b'[') => self.parse_array(depth, path),
            Some(b'{') => self.parse_object(depth, path),
            Some(b'-' | b'0'..=b'9') => self.parse_number(path),
            Some(_) => Err(self.malformed(path, "expected a JSON value")),
            None => Err(self.malformed(path, "unexpected end of input")),
        }
    }

    fn parse_array(&mut self, depth: usize, path: &str) -> Result<Value, RulePackageError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume_byte(b']') {
            return Ok(Value::Array(values));
        }
        loop {
            if let Some((_, maximum)) = self
                .collection_limits
                .iter()
                .find(|(limited_path, _)| *limited_path == path)
            {
                if values.len() >= *maximum {
                    return Err(RulePackageError::QuotaExceeded {
                        path: path.to_string(),
                        actual: maximum + 1,
                        maximum: *maximum,
                    });
                }
            }
            let index = values.len();
            values.push(self.parse_value(depth + 1, &pointer_index(path, index))?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                break;
            }
            if !self.consume_byte(b',') {
                return Err(self.malformed(path, "expected ',' or ']' in array"));
            }
            self.skip_whitespace();
        }
        Ok(Value::Array(values))
    }

    fn parse_object(&mut self, depth: usize, path: &str) -> Result<Value, RulePackageError> {
        self.offset += 1;
        self.skip_whitespace();
        let mut values = Map::new();
        let mut seen = BTreeSet::new();
        if self.consume_byte(b'}') {
            return Ok(Value::Object(values));
        }
        loop {
            if self.peek_byte() != Some(b'"') {
                return Err(self.malformed(path, "expected a string object key"));
            }
            let key = self.parse_string(&format!("{path}/<key>"))?;
            if !seen.insert(key.clone()) {
                return Err(RulePackageError::DuplicateJsonKey {
                    path: path.to_string(),
                    key,
                });
            }
            self.skip_whitespace();
            if !self.consume_byte(b':') {
                return Err(self.malformed(path, "expected ':' after object key"));
            }
            self.skip_whitespace();
            let child_path = pointer_key(path, &key);
            let value = self.parse_value(depth + 1, &child_path)?;
            values.insert(key, value);
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                break;
            }
            if !self.consume_byte(b',') {
                return Err(self.malformed(path, "expected ',' or '}' in object"));
            }
            self.skip_whitespace();
        }
        Ok(Value::Object(values))
    }

    fn parse_number(&mut self, path: &str) -> Result<Value, RulePackageError> {
        let start = self.offset;
        let negative = self.consume_byte(b'-');
        let Some(first) = self.peek_byte() else {
            return Err(self.malformed(path, "incomplete JSON number"));
        };
        match first {
            b'0' => {
                self.offset += 1;
                if self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    return Err(self.malformed(path, "leading zero in JSON number"));
                }
            }
            b'1'..=b'9' => {
                self.offset += 1;
                while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.offset += 1;
                }
            }
            _ => return Err(self.malformed(path, "invalid JSON number")),
        }
        if matches!(self.peek_byte(), Some(b'.' | b'e' | b'E')) {
            while self.peek_byte().is_some_and(|byte| {
                byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
            }) {
                self.offset += 1;
            }
            return Err(RulePackageError::JsonIntegerOutOfRange {
                path: path.to_string(),
                value: bounded_token(&self.input[start..self.offset]),
            });
        }
        let token = &self.input[start..self.offset];
        let digits = token.trim_start_matches('-');
        let magnitude = digits.parse::<u64>().ok();
        let Some(magnitude) = magnitude else {
            return Err(RulePackageError::JsonIntegerOutOfRange {
                path: path.to_string(),
                value: bounded_token(token),
            });
        };
        if magnitude > MAX_SAFE_JSON_INTEGER {
            return Err(RulePackageError::JsonIntegerOutOfRange {
                path: path.to_string(),
                value: bounded_token(token),
            });
        }
        if negative {
            let value = -(magnitude as i64);
            Ok(Value::Number(Number::from(value)))
        } else {
            Ok(Value::Number(Number::from(magnitude)))
        }
    }

    fn parse_string(&mut self, path: &str) -> Result<String, RulePackageError> {
        debug_assert_eq!(self.peek_byte(), Some(b'"'));
        self.offset += 1;
        let mut output = String::new();
        loop {
            let Some(byte) = self.peek_byte() else {
                return Err(self.malformed(path, "unterminated JSON string"));
            };
            match byte {
                b'"' => {
                    self.offset += 1;
                    validate_string(&output, path)?;
                    return Ok(output);
                }
                b'\\' => {
                    self.offset += 1;
                    self.parse_escape(path, &mut output)?;
                }
                0x00..=0x1f => {
                    return Err(self.malformed(path, "unescaped control character in JSON string"));
                }
                0x20..=0x7f => {
                    output.push(byte as char);
                    self.offset += 1;
                }
                _ => {
                    let character = self.input[self.offset..]
                        .chars()
                        .next()
                        .expect("input is valid UTF-8");
                    output.push(character);
                    self.offset += character.len_utf8();
                }
            }
            if output.len() > JSON_MAX_STRING_BYTES {
                return Err(RulePackageError::QuotaExceeded {
                    path: path.to_string(),
                    actual: output.len(),
                    maximum: JSON_MAX_STRING_BYTES,
                });
            }
        }
    }

    fn parse_escape(&mut self, path: &str, output: &mut String) -> Result<(), RulePackageError> {
        let Some(escaped) = self.peek_byte() else {
            return Err(self.malformed(path, "incomplete JSON escape"));
        };
        self.offset += 1;
        match escaped {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let first = self.parse_hex_quad(path)?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    if !self.consume_byte(b'\\') || !self.consume_byte(b'u') {
                        return Err(self.malformed(path, "unpaired high surrogate"));
                    }
                    let second = self.parse_hex_quad(path)?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(self.malformed(path, "unpaired high surrogate"));
                    }
                    0x10000 + (((u32::from(first) - 0xd800) << 10) | (u32::from(second) - 0xdc00))
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(self.malformed(path, "unpaired low surrogate"));
                } else {
                    u32::from(first)
                };
                output.push(
                    char::from_u32(scalar)
                        .ok_or_else(|| self.malformed(path, "invalid Unicode scalar"))?,
                );
            }
            _ => return Err(self.malformed(path, "unsupported JSON escape")),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self, path: &str) -> Result<u16, RulePackageError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let Some(byte) = self.peek_byte() else {
                return Err(self.malformed(path, "incomplete Unicode escape"));
            };
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a' + 10),
                b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return Err(self.malformed(path, "invalid Unicode escape")),
            };
            value = value * 16 + digit;
            self.offset += 1;
        }
        Ok(value)
    }

    fn expect_literal(&mut self, literal: &str, path: &str) -> Result<(), RulePackageError> {
        if self.input[self.offset..].starts_with(literal) {
            self.offset += literal.len();
            Ok(())
        } else {
            Err(self.malformed(path, "invalid JSON literal"))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }

    fn malformed(&self, path: &str, reason: &str) -> RulePackageError {
        RulePackageError::MalformedJson {
            path: path.to_string(),
            offset: self.offset,
            reason: reason.to_string(),
        }
    }
}

fn bounded_token(value: &str) -> String {
    const LIMIT: usize = 64;
    if value.len() <= LIMIT {
        value.to_string()
    } else {
        format!("{}...", &value[..LIMIT])
    }
}
