//! The developer content source as Engine live-debug commands, so a page,
//! an agent or crew-services reads and saves source content the way the
//! ContentSource service does.
use super::*;

pub(super) const READ_COMMAND: &str = "engine.content.source.read";
pub(super) const EDIT_COMMAND: &str = "engine.content.source.edit";

#[derive(serde::Serialize)]
struct ReadAnswer<'a> {
    path: &'a str,
    sha256: String,
    text: String,
}

#[derive(serde::Serialize)]
struct EditAnswer<'a> {
    path: &'a str,
    sha256: String,
}

fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn digest(hex: &str) -> Option<[u8; 32]> {
    let mut digest = [0_u8; 32];
    if hex.len() != 64 {
        return None;
    }
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(hex.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(digest)
}

impl CsharpProductRuntime {
    /// Answers `engine.content.source.read <path>` and
    /// `engine.content.source.edit <path> <sha256> <edits>`, where `edits` is
    /// the rest of the line; none for any other command.
    pub(super) fn execute_content_source_debug(
        &mut self,
        command: &str,
    ) -> Option<ProductHostDebugResult> {
        let command = command.trim();
        let (name, rest) = command
            .split_once(char::is_whitespace)
            .unwrap_or((command, ""));
        let rest = rest.trim();
        let answer = match name {
            READ_COMMAND => {
                if rest.is_empty() {
                    return Some(usage(READ_COMMAND, "<path>"));
                }
                self.services
                    .read_content_source(rest)
                    .map_err(|error| format!("{}: {}", error.code(), error.detail()))
                    .and_then(|(bytes, sha256)| {
                        json(&ReadAnswer {
                            path: rest,
                            sha256: hex(&sha256),
                            text: String::from_utf8_lossy(&bytes).into_owned(),
                        })
                    })
            }
            EDIT_COMMAND => {
                let mut parts = rest.splitn(3, char::is_whitespace);
                let (Some(path), Some(sha256), Some(edits)) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    return Some(usage(EDIT_COMMAND, "<path> <sha256> <edits>"));
                };
                let Some(expected) = digest(sha256) else {
                    return Some(ProductHostDebugResult::new(
                        false,
                        "CONTENT_SOURCE_STALE: sha256 must be the 64 hex digits a read returned"
                            .to_owned(),
                    ));
                };
                csharp_engine_services::parse_content_source_edits(edits)
                    .and_then(|edits| self.services.edit_content_source(path, expected, &edits))
                    .map_err(|error| format!("{}: {}", error.code(), error.detail()))
                    .and_then(|sha256| {
                        json(&EditAnswer {
                            path,
                            sha256: hex(&sha256),
                        })
                    })
            }
            _ => return None,
        };
        Some(match answer {
            Ok(message) => ProductHostDebugResult::new(true, message),
            Err(message) => ProductHostDebugResult::new(false, message),
        })
    }
}

fn usage(name: &str, parameters: &str) -> ProductHostDebugResult {
    ProductHostDebugResult::new(false, format!("Usage: {name} {parameters}"))
}

fn json(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_text_round_trips() {
        let value: [u8; 32] = std::array::from_fn(|index| index as u8 * 7);
        assert_eq!(digest(&hex(&value)), Some(value));
        assert_eq!(digest("ab"), None);
        assert_eq!(digest(&"zz".repeat(32)), None);
    }
}
