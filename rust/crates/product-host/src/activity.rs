//! When someone last used this host. A person or an agent driving it (input,
//! a control claim, a lifecycle or control call, a live-debug command, a page
//! attaching) is activity; a page or browser merely pulling frames is not, so
//! an abandoned headless or playtest browser does not keep a host alive.
//! `rusty dev` reads the mark's file time to stop idle sessions.

use std::{
    fs::{self, File},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// The file is rewritten at most this often.
const WRITE_INTERVAL_MS: u64 = 1_000;

pub struct ProductHostActivity {
    path: PathBuf,
    written_ms: AtomicU64,
}

impl ProductHostActivity {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            written_ms: AtomicU64::new(0),
        }
    }

    /// Records activity now, writing the file at most once a second.
    pub fn touch(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        let written = self.written_ms.load(Ordering::Relaxed);
        if now.saturating_sub(written) < WRITE_INTERVAL_MS
            || self
                .written_ms
                .compare_exchange(written, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        // The file's modification time is the mark; a failed write leaves the
        // previous one, which only makes the session look idler.
        let _ = File::create(&self.path)
            .and_then(|file| file.set_modified(SystemTime::now()))
            .or_else(|_| fs::write(&self.path, b""));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_writes_the_mark_at_most_once_a_second() {
        let path = std::env::temp_dir().join(format!("rusty-activity-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let activity = ProductHostActivity::new(path.clone());
        activity.touch();
        let first = fs::metadata(&path).unwrap().modified().unwrap();
        fs::remove_file(&path).unwrap();
        activity.touch();
        assert!(
            !path.exists(),
            "a second touch within the interval writes nothing"
        );
        activity.written_ms.store(0, Ordering::Relaxed);
        activity.touch();
        assert!(fs::metadata(&path).unwrap().modified().unwrap() >= first);
        fs::remove_file(&path).unwrap();
    }
}
