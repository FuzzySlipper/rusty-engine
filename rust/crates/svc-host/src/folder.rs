//! The platform's folder picker, shown from its own thread so the caller
//! keeps running while the player chooses: xdg-desktop-portal on Linux
//! (drawn by the desktop's own portal backend), the Windows folder dialog on
//! Windows.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    thread,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChoiceState {
    /// The dialog is open.
    Pending,
    Chosen(PathBuf),
    Cancelled,
    /// No picker could be shown; the text says why.
    Unavailable(String),
}

/// One request for a folder. Dropping it does not close an open dialog; the
/// player's answer is then discarded.
pub struct FolderChoice {
    outcome: Arc<Mutex<FolderChoiceState>>,
}

impl FolderChoice {
    pub fn start(title: &str) -> Self {
        let outcome = Arc::new(Mutex::new(FolderChoiceState::Pending));
        let answer = Arc::clone(&outcome);
        let title = title.to_owned();
        let started = thread::Builder::new()
            .name("rusty-folder-choice".to_owned())
            .spawn(move || {
                let chosen = pick(&title);
                *answer.lock().unwrap_or_else(PoisonError::into_inner) = chosen;
            });
        if let Err(error) = started {
            *outcome.lock().unwrap_or_else(PoisonError::into_inner) =
                FolderChoiceState::Unavailable(format!(
                    "the picker's thread did not start: {error}"
                ));
        }
        Self { outcome }
    }

    pub fn snapshot(&self) -> FolderChoiceState {
        self.outcome
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(target_os = "linux")]
fn pick(title: &str) -> FolderChoiceState {
    use ashpd::desktop::{file_chooser::SelectedFiles, ResponseError};

    let answer = pollster::block_on(async {
        SelectedFiles::open_file()
            .title(title)
            .directory(true)
            .multiple(false)
            .modal(true)
            .send()
            .await?
            .response()
    });
    match answer {
        Ok(files) => match files.uris().first() {
            Some(uri) => match file_uri_path(uri.as_str()) {
                Some(path) => FolderChoiceState::Chosen(path),
                None => FolderChoiceState::Unavailable(format!(
                    "the portal answered `{uri}`, which is not a local folder"
                )),
            },
            None => FolderChoiceState::Cancelled,
        },
        Err(ashpd::Error::Response(ResponseError::Cancelled)) => FolderChoiceState::Cancelled,
        Err(error) => FolderChoiceState::Unavailable(format!(
            "the desktop's folder picker (xdg-desktop-portal) is unavailable: {error}"
        )),
    }
}

#[cfg(windows)]
fn pick(title: &str) -> FolderChoiceState {
    // The dialog itself returns nothing but a folder or a cancel.
    match rfd::FileDialog::new().set_title(title).pick_folder() {
        Some(path) => FolderChoiceState::Chosen(path),
        None => FolderChoiceState::Cancelled,
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn pick(_title: &str) -> FolderChoiceState {
    FolderChoiceState::Unavailable("this platform has no folder picker".to_owned())
}

/// The local path a portal's `file://` URI names, percent-decoded.
#[cfg(target_os = "linux")]
fn file_uri_path(uri: &str) -> Option<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let encoded = uri.strip_prefix("file://")?;
    // A host part other than empty or localhost is another machine.
    let encoded = encoded.strip_prefix("localhost").unwrap_or(encoded);
    if !encoded.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut rest = encoded.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'%' && tail.len() >= 2 {
            let hex = std::str::from_utf8(&tail[..2]).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn a_portal_uri_names_its_decoded_local_path() {
        assert_eq!(
            file_uri_path("file:///home/player/Daggerfall%20Files/ARENA2"),
            Some(PathBuf::from("/home/player/Daggerfall Files/ARENA2"))
        );
        assert_eq!(
            file_uri_path("file://localhost/data/%C3%A9t%C3%A9"),
            Some(PathBuf::from("/data/été"))
        );
        assert_eq!(file_uri_path("file://server/share"), None);
        assert_eq!(file_uri_path("https://example.com/"), None);
    }
}
