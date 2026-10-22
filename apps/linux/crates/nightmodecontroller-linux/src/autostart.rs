//! Start-at-login via a freedesktop autostart `.desktop` entry.
//!
//! Implements [`LoginItemService`]: the entry at
//! `~/.config/autostart/nightmodecontroller-linux.desktop` is the system-owned
//! registration, and the settings flag is only the user's intent.

use nmc_core::traits::LoginItemService;
use std::path::PathBuf;

const DESKTOP_FILE_NAME: &str = "nightmodecontroller-linux.desktop";

pub fn autostart_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("autostart")
}

pub fn autostart_file() -> PathBuf {
    autostart_dir().join(DESKTOP_FILE_NAME)
}

fn desktop_entry_text(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Night Mode Controller\n\
         Comment=External-display brightness and scheduled colour temperature\n\
         Exec={exec}\n\
         Icon=nightmodecontroller-linux\n\
         Terminal=false\n\
         Categories=Utility;\n\
         X-GNOME-Autostart-enabled=true\n\
         X-KDE-autostart-after=panel\n"
    )
}

pub struct AutostartService {
    file_path: PathBuf,
}

impl AutostartService {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn default_path() -> Self {
        Self::new(autostart_file())
    }
}

impl LoginItemService for AutostartService {
    fn is_enabled(&self) -> bool {
        self.file_path.is_file()
    }

    fn requires_approval(&self) -> bool {
        // Linux has no login-item approval flow.
        false
    }

    fn set_enabled(&mut self, enabled: bool) -> bool {
        if enabled {
            let exec = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "nightmodecontroller-linux".to_string());
            if let Some(parent) = self.file_path.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    return false;
                }
            }
            std::fs::write(&self.file_path, desktop_entry_text(&exec)).is_ok()
        } else {
            match std::fs::remove_file(&self.file_path) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
                Err(_) => false,
            }
        }
    }

    fn open_settings(&self) {
        open_dir(&autostart_dir());
    }
}

/// Shows a directory in the file manager. The directory is created first:
/// xdg-open on a missing path fails silently, and ~/.config/autostart often
/// does not exist until something registers a login item.
fn open_dir(dir: &std::path::Path) {
    ensure_dir(dir);
    // Best effort: show the directory in the file manager.
    let _ = open_command(dir).spawn();
}

/// Creates the directory when missing so the opener never targets a path
/// that fails silently.
fn ensure_dir(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
}

/// xdg-open invocation for a directory, detached from our stdio.
fn open_command(dir: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_service(name: &str) -> (AutostartService, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "nmc-autostart-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join(DESKTOP_FILE_NAME);
        (AutostartService::new(path), dir)
    }

    #[test]
    fn registers_and_unregisters() {
        let (mut service, dir) = temp_service("basic");
        assert!(!service.is_enabled());
        assert!(!service.requires_approval());
        assert!(service.set_enabled(true));
        assert!(service.is_enabled());
        let text = std::fs::read_to_string(&service.file_path).unwrap();
        assert!(text.contains("[Desktop Entry]"));
        assert!(text.contains("X-GNOME-Autostart-enabled=true"));
        assert!(service.set_enabled(false));
        assert!(!service.is_enabled());
        // Removing twice is still success.
        assert!(service.set_enabled(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opener_targets_the_directory() {
        let dir = std::env::temp_dir().join("nmc-autostart-opener");
        let command = open_command(&dir);
        assert_eq!(command.get_program(), "xdg-open");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], dir.as_os_str());
    }

    #[test]
    fn missing_directory_is_created_before_open() {
        // Regression: xdg-open on a missing ~/.config/autostart failed
        // silently. open_dir creates it first (the spawn itself is best
        // effort and untestable headless).
        let dir = std::env::temp_dir().join(format!(
            "nmc-autostart-mkdir-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!dir.exists());
        ensure_dir(&dir);
        assert!(dir.is_dir());
        // Idempotent on an existing directory.
        ensure_dir(&dir);
        assert!(dir.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
