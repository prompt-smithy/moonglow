//! Application log in the XDG state directory.
//!
//! The long-running instance captures everything it prints to stderr
//! (startup lines, backend warnings, panic messages) into
//! `~/.local/state/nightmodecontroller-linux/nightmodecontroller-linux.log` (or
//! `$XDG_STATE_HOME/...`). One-shot CLI invocations never touch it: their
//! output belongs to the terminal.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Rotate past this size: the log is diagnostics, not history.
const MAX_LOG_BYTES: u64 = 2_000_000;

pub fn default_path() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("nightmodecontroller-linux")
        .join("nightmodecontroller-linux.log")
}

/// Opens the log for appending, rotating a full one aside first. Writes a
/// session banner so consecutive runs stay separable.
pub fn open_log(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    rotate_if_full(path);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "=== nightmodecontroller-linux {} starting {} (pid {}) ===",
        env!("CARGO_PKG_VERSION"),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        std::process::id()
    )?;
    Ok(file)
}

fn rotate_if_full(path: &Path) {
    let full = std::fs::metadata(path)
        .map(|m| m.len() > MAX_LOG_BYTES)
        .unwrap_or(false);
    if !full {
        return;
    }
    let backup = path.with_extension("log.1");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::rename(path, &backup);
}

/// Points stderr at the open log file. Stdout stays on the terminal: CLI
/// output is user-facing, diagnostics are not.
pub fn redirect_stderr(file: &File) {
    let _ = rustix::stdio::dup2_stderr(file);
}

/// Full setup for the long-running instance. Returns the path for the
/// startup line, or None when logging is unavailable (the app still runs).
pub fn init() -> Option<PathBuf> {
    let path = default_path();
    match open_log(&path) {
        Ok(file) => {
            redirect_stderr(&file);
            // Closing our copy is fine: stderr holds its own dup.
            Some(path)
        }
        Err(e) => {
            eprintln!("log unavailable ({}): {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nmc-logtest-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn log_path_lives_in_state_dir() {
        let path = default_path();
        assert!(
            path.ends_with("nightmodecontroller-linux/nightmodecontroller-linux.log"),
            "unexpected: {}",
            path.display()
        );
    }

    #[test]
    fn open_log_creates_and_banners() {
        let dir = temp_path("new");
        let path = dir.join("nested").join("test.log");
        let _file = open_log(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("starting"), "banner: {text}");
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        // Second run appends a second banner instead of truncating.
        let _file = open_log(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_log_rotates_aside() {
        let dir = temp_path("rotate");
        let path = dir.join("test.log");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize + 1]).unwrap();
        let _file = open_log(&path).unwrap();
        // Old content moved to the backup; the live log starts fresh.
        let backup = path.with_extension("log.1");
        assert_eq!(std::fs::metadata(&backup).unwrap().len(), MAX_LOG_BYTES + 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("starting"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
