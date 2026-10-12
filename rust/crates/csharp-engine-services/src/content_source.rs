//! Developer-only reads and JSON pointer edits of a product's source content,
//! in a `rusty dev` session whose staged manifest names the source content
//! root. An edit writes the source file and its staged copy, so `rusty dev`
//! sees bytes it already staged and neither restages nor reloads.

use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
};

use csharp_engine_abi::*;
use runtime_diagnostics::{
    RuntimeDiagnosticDisposition, RuntimeDiagnosticEvent, RuntimeDiagnosticSeverity,
    RuntimeDiagnosticsSink,
};
use sha2::{Digest, Sha256};

use crate::composition::{borrowed_utf8, CsharpEngineServicesError, ABI_OK};
use crate::operation_diagnostics::{clear_receipt, refuse, BorrowedResult};

mod json_edit;
pub use json_edit::JsonEdit;

/// The source content root `rusty dev` staged from, and the staged content
/// root the running product reads.
#[derive(Debug, Clone)]
pub struct ContentSourceRoots {
    pub source: PathBuf,
    pub staged: PathBuf,
}

/// The developer content source of one runtime: none outside `rusty dev`.
#[derive(Debug, Clone, Default)]
pub struct ContentSource {
    roots: Option<ContentSourceRoots>,
}

fn error(code: &'static str, detail: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, detail)
}

impl ContentSource {
    pub fn new(roots: Option<ContentSourceRoots>) -> Self {
        Self { roots }
    }

    fn roots(&self) -> Result<&ContentSourceRoots, CsharpEngineServicesError> {
        self.roots.as_ref().ok_or_else(|| {
            error(
                "CONTENT_SOURCE_UNAVAILABLE",
                "the content source is only available in a `rusty dev` session",
            )
        })
    }

    /// The source bytes of the content-root-relative `path` and their SHA-256.
    pub fn read(&self, path: &str) -> Result<(Vec<u8>, [u8; 32]), CsharpEngineServicesError> {
        let file = self.roots()?.source.join(path);
        let bytes = fs::read(&file).map_err(|failure| {
            error(
                "CONTENT_SOURCE_MISSING",
                format!("`{path}` could not be read from the source content: {failure}"),
            )
        })?;
        let sha256 = Sha256::digest(&bytes).into();
        Ok((bytes, sha256))
    }

    /// Applies `edits` to the source JSON at `path`, which must still have
    /// the SHA-256 `expected`, keeping every other byte; writes the result to
    /// the staged copy and then the source file, each atomically. Returns the
    /// new SHA-256.
    pub fn edit(
        &self,
        path: &str,
        expected: [u8; 32],
        edits: &[JsonEdit],
    ) -> Result<[u8; 32], CsharpEngineServicesError> {
        let roots = self.roots()?;
        let (bytes, sha256) = self.read(path)?;
        if sha256 != expected {
            return Err(error(
                "CONTENT_SOURCE_STALE",
                format!("`{path}` changed since it was read; read it again"),
            ));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| error("CONTENT_SOURCE_NOT_JSON", format!("`{path}` is not UTF-8 JSON")))?;
        let edited = json_edit::apply(text, edits).map_err(|failure| match failure {
            json_edit::JsonEditError::Text(detail) => {
                error("CONTENT_SOURCE_NOT_JSON", format!("`{path}`: {detail}"))
            }
            json_edit::JsonEditError::Pointer(detail) => {
                error("CONTENT_SOURCE_POINTER", format!("`{path}`: {detail}"))
            }
        })?;
        // The staged copy first: when the watcher sees the source change, the
        // staged bytes already match it.
        write_atomically(&roots.staged.join(path), edited.as_bytes())?;
        write_atomically(&roots.source.join(path), edited.as_bytes())?;
        Ok(Sha256::digest(edited.as_bytes()).into())
    }
}

fn write_atomically(file: &Path, bytes: &[u8]) -> Result<(), CsharpEngineServicesError> {
    let failed = |failure: std::io::Error| {
        error(
            "CONTENT_SOURCE_WRITE",
            format!("`{}` could not be written: {failure}", file.display()),
        )
    };
    let name = file
        .file_name()
        .ok_or_else(|| error("CONTENT_SOURCE_WRITE", "the path names no file"))?;
    let temporary = file.with_file_name(format!(".{}.rusty-edit", name.to_string_lossy()));
    fs::write(&temporary, bytes).map_err(failed)?;
    fs::rename(&temporary, file).map_err(|failure| {
        let _ = fs::remove_file(&temporary);
        failed(failure)
    })
}

/// RFC 6902 operations limited to the ones a pointer edit needs: `replace`,
/// `add` and `remove`, each with its `path` (a JSON pointer).
pub fn parse_edits(text: &str) -> Result<Vec<JsonEdit>, CsharpEngineServicesError> {
    #[derive(serde::Deserialize)]
    #[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
    enum Operation {
        Replace { path: String, value: serde_json::Value },
        Add { path: String, value: serde_json::Value },
        Remove { path: String },
    }
    let operations: Vec<Operation> = serde_json::from_str(text).map_err(|failure| {
        error(
            "CONTENT_SOURCE_EDITS",
            format!("edits must be a JSON array of replace, add and remove operations: {failure}"),
        )
    })?;
    Ok(operations
        .into_iter()
        .map(|operation| match operation {
            Operation::Replace { path, value } => JsonEdit::Replace {
                pointer: path,
                value,
            },
            Operation::Add { path, value } => JsonEdit::Add {
                pointer: path,
                value,
            },
            Operation::Remove { path } => JsonEdit::Remove { pointer: path },
        })
        .collect())
}

/// SHA-256 as the four big-endian words the ABI carries.
pub fn sha256_words(digest: &[u8; 32]) -> NativeContentSha256 {
    let word = |index: usize| {
        u64::from_be_bytes(digest[index * 8..index * 8 + 8].try_into().expect("8 bytes"))
    };
    NativeContentSha256 {
        word0: word(0),
        word1: word(1),
        word2: word(2),
        word3: word(3),
    }
}

fn digest_of(words: NativeContentSha256) -> [u8; 32] {
    let mut digest = [0; 32];
    for (index, word) in [words.word0, words.word1, words.word2, words.word3]
        .into_iter()
        .enumerate()
    {
        digest[index * 8..index * 8 + 8].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

pub(crate) struct RuntimeContentSourceBridge {
    source: ContentSource,
    diagnostics: Option<RuntimeDiagnosticsSink>,
    /// Backing of the latest read result.
    borrowed: BorrowedResult,
}

impl RuntimeContentSourceBridge {
    pub(crate) fn new(diagnostics: Option<RuntimeDiagnosticsSink>) -> Self {
        Self {
            source: ContentSource::default(),
            diagnostics,
            borrowed: BorrowedResult::default(),
        }
    }

    pub(crate) fn bind(&mut self, source: ContentSource) {
        self.source = source;
    }

    pub(crate) fn read(&self, path: &str) -> Result<(Vec<u8>, [u8; 32]), CsharpEngineServicesError> {
        self.source.read(path)
    }

    /// Edits the source and reports the write in the runtime diagnostics, so
    /// the session log shows what was saved.
    pub(crate) fn edit(
        &self,
        path: &str,
        expected: [u8; 32],
        edits: &[JsonEdit],
    ) -> Result<[u8; 32], CsharpEngineServicesError> {
        let sha256 = self.source.edit(path, expected, edits)?;
        if let Some(sink) = &self.diagnostics {
            if let Ok(event) = RuntimeDiagnosticEvent::new(
                RuntimeDiagnosticSeverity::Info,
                RuntimeDiagnosticDisposition::Accepted,
                "content-source",
                "CONTENT_SOURCE_WRITTEN",
                format!("wrote the source and staged copy of `{path}`"),
            ) {
                let _ = sink.publish(event);
            }
        }
        Ok(sha256)
    }
}

pub(crate) fn api(bridge: &mut RuntimeContentSourceBridge) -> NativeContentSourceApi {
    NativeContentSourceApi {
        context: (bridge as *mut RuntimeContentSourceBridge).cast(),
        read,
        edit,
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: *const NativeContentSourceReadRequest,
    result: *mut NativeByteResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentSourceBridge>() };
    let read = unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
        .and_then(|path| bridge.read(path));
    let (bytes, _) = match read {
        Ok(read) => read,
        Err(refusal) => return refuse(&refusal, error),
    };
    unsafe {
        *result = NativeByteResult {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
    }
    bridge.borrowed.hold(bytes);
    ABI_OK
}

unsafe extern "C" fn edit(
    context: *mut c_void,
    request: *const NativeContentSourceEditRequest,
    result: *mut NativeContentSha256,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let bridge = unsafe { &mut *context.cast::<RuntimeContentSourceBridge>() };
    let edited = unsafe { borrowed_utf8(request.path.bytes, request.path.len, "content path") }
        .and_then(|path| {
            let edits = unsafe { borrowed_utf8(request.edits.bytes, request.edits.len, "edits") }
                .and_then(parse_edits)?;
            bridge.edit(path, digest_of(request.expected_sha256), &edits)
        });
    match edited {
        Ok(sha256) => {
            unsafe { *result = sha256_words(&sha256) };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "{\n  \"marker\": {\n    \"size\": 0.8,\n    \"spin\": true\n  }\n}\n";

    fn session() -> (tempfile::TempDir, ContentSource) {
        let directory = tempfile::tempdir().unwrap();
        let roots = ContentSourceRoots {
            source: directory.path().join("source"),
            staged: directory.path().join("staged"),
        };
        for root in [&roots.source, &roots.staged] {
            fs::create_dir_all(root.join("tuning")).unwrap();
            fs::write(root.join("tuning/marker.json"), TEXT).unwrap();
        }
        fs::write(roots.source.join("notes.txt"), "not json").unwrap();
        (directory, ContentSource::new(Some(roots)))
    }

    fn code(result: Result<impl std::fmt::Debug, CsharpEngineServicesError>) -> String {
        result.unwrap_err().code().to_owned()
    }

    #[test]
    fn an_edit_changes_one_line_of_the_source_and_its_staged_copy() {
        let (directory, source) = session();
        let (bytes, sha256) = source.read("tuning/marker.json").unwrap();
        assert_eq!(bytes, TEXT.as_bytes());
        let edits = parse_edits(r#"[{"op":"replace","path":"/marker/size","value":1.25}]"#).unwrap();
        let written = source.edit("tuning/marker.json", sha256, &edits).unwrap();
        let expected = TEXT.replace("0.8", "1.25");
        for root in ["source", "staged"] {
            let file = directory.path().join(root).join("tuning/marker.json");
            assert_eq!(fs::read_to_string(file).unwrap(), expected);
        }
        assert_eq!(written, <[u8; 32]>::from(Sha256::digest(expected.as_bytes())));
        assert!(!directory.path().join("source/tuning/.marker.json.rusty-edit").exists());
        assert_eq!(digest_of(sha256_words(&written)), written);
    }

    #[test]
    fn refusals_name_their_cause() {
        let (_directory, source) = session();
        let (_, sha256) = source.read("tuning/marker.json").unwrap();
        let replace = parse_edits(r#"[{"op":"replace","path":"/marker/size","value":2}]"#).unwrap();
        assert_eq!(code(source.edit("tuning/marker.json", [0; 32], &replace)), "CONTENT_SOURCE_STALE");
        assert_eq!(code(source.read("tuning/none.json")), "CONTENT_SOURCE_MISSING");
        let (_, notes) = source.read("notes.txt").unwrap();
        assert_eq!(code(source.edit("notes.txt", notes, &replace)), "CONTENT_SOURCE_NOT_JSON");
        let missing = parse_edits(r#"[{"op":"replace","path":"/marker/none","value":2}]"#).unwrap();
        assert_eq!(code(source.edit("tuning/marker.json", sha256, &missing)), "CONTENT_SOURCE_POINTER");
        assert_eq!(code(parse_edits(r#"[{"op":"move","path":"/a","from":"/b"}]"#)), "CONTENT_SOURCE_EDITS");
        let packaged = ContentSource::default();
        assert_eq!(code(packaged.read("tuning/marker.json")), "CONTENT_SOURCE_UNAVAILABLE");
        assert_eq!(code(packaged.edit("tuning/marker.json", sha256, &replace)), "CONTENT_SOURCE_UNAVAILABLE");
    }
}
