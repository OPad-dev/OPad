//! The HE debug firmware's log lines (pad LogEvents tagged `he.*`, see
//! firmware/main/input/he_debug.h), appended to `<data>/he-debug.log`. The
//! in-memory log hub keeps 2000 entries, a few minutes of a debug session;
//! the file keeps the whole session. It rotates once at 50 MB (the previous
//! file becomes `he-debug.log.1`), so a forgotten debug pad cannot fill the
//! disk. Normal firmware sends no `he.*` lines, so the file never appears.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const MAX_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Default)]
pub struct HeDebugLog {
    dir: Option<PathBuf>,
    file: Option<File>,
    written: u64,
}

impl HeDebugLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// A log file in `dir` instead of the data directory (tests)
    pub fn in_dir(dir: PathBuf) -> Self {
        Self {
            dir: Some(dir),
            ..Self::default()
        }
    }

    pub fn is_debug_tag(tag: &str) -> bool {
        tag.starts_with("he.")
    }

    fn path(&self) -> Option<PathBuf> {
        let dir = match &self.dir {
            Some(d) => d.clone(),
            None => opad_model::paths::data_dir().ok()?,
        };
        Some(dir.join("he-debug.log"))
    }

    /// Append one line; failures are dropped (a debug log must never stop
    /// the daemon)
    pub fn write(&mut self, level: &str, tag: &str, message: &str) {
        let Some(path) = self.path() else { return };
        if self.file.is_none() || self.written >= MAX_BYTES {
            if self.written >= MAX_BYTES {
                self.file = None;
                let _ = std::fs::rename(&path, path.with_extension("log.1"));
            }
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(f) => {
                    self.written = f.metadata().map(|m| m.len()).unwrap_or(0);
                    self.file = Some(f);
                }
                Err(_) => return,
            }
        }
        let line = format!(
            "{} {:<5} {:<8} {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            level,
            tag,
            message
        );
        if let Some(f) = self.file.as_mut() {
            if f.write_all(line.as_bytes()).is_ok() {
                self.written += line.len() as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_debug_lines_and_rotates() {
        let dir = std::env::temp_dir().join(format!("opad-he-debug-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut log = HeDebugLog::in_dir(dir.clone());
        log.write("INFO", "he.edge", "[     1234 ms] K2 DOWN A>P raw+154");
        let text = std::fs::read_to_string(dir.join("he-debug.log")).unwrap();
        assert!(text.contains("he.edge") && text.contains("K2 DOWN A>P raw+154"));
        // Past the limit, the file is moved aside and a fresh one started
        log.written = MAX_BYTES;
        log.write("INFO", "he.sec", "after rotation");
        assert!(dir.join("he-debug.log.1").exists());
        let fresh = std::fs::read_to_string(dir.join("he-debug.log")).unwrap();
        assert!(fresh.contains("after rotation") && !fresh.contains("he.edge"));
        assert!(HeDebugLog::is_debug_tag("he.pre") && !HeDebugLog::is_debug_tag("firmware"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
