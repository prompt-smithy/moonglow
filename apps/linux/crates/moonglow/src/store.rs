//! Loads and persists [`AppSettings`] as JSON under the XDG config directory.
//!
//! Mirrors Swift `SettingsStore`: a pure persistence adapter where the shared
//! copy of settings is owned by [`StateManager`]. A background worker coalesces
//! writes and flushes before exit; filesystem failures never crash the app.

use moonglow_core::settings::AppSettings;
use moonglow_core::traits::SettingsPersistence;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// `~/.config/moonglow/settings.json` (or
/// `$XDG_CONFIG_HOME/moonglow/settings.json`).
pub fn default_file_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".config")
    });
    base.join("moonglow").join("settings.json")
}

pub struct SettingsStore {
    pub file_path: PathBuf,
    writer: Option<mpsc::Sender<WriteRequest>>,
}

enum WriteRequest {
    Save(AppSettings),
    Flush(mpsc::Sender<()>),
}

fn write_settings(path: &std::path::Path, settings: &AppSettings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&settings.normalized())?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)
}

fn run_writer(path: PathBuf, receiver: mpsc::Receiver<WriteRequest>) {
    let mut pending = None;
    loop {
        let request = if pending.is_some() {
            receiver.recv_timeout(Duration::from_millis(250))
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match request {
            Ok(WriteRequest::Save(settings)) => pending = Some(settings),
            Ok(WriteRequest::Flush(reply)) => {
                if let Some(settings) = pending.take() {
                    let _ = write_settings(&path, &settings);
                }
                let _ = reply.send(());
            }
            Err(error) => {
                if let Some(settings) = pending.take() {
                    let _ = write_settings(&path, &settings);
                }
                if error == mpsc::RecvTimeoutError::Disconnected {
                    return;
                }
            }
        }
    }
}

impl SettingsStore {
    pub fn new(file_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let path = file_path.clone();
        let writer = std::thread::Builder::new()
            .name("moonglow-settings".into())
            .spawn(move || run_writer(path, receiver))
            .ok()
            .map(|_| sender);
        Self { file_path, writer }
    }

    pub fn default_path() -> Self {
        Self::new(default_file_path())
    }

    /// Moves an unparseable settings file aside, replacing any previous
    /// preservation. Best effort, like everything else here.
    fn preserve_corrupt_file(&self) {
        let corrupt = self.file_path.with_extension("json.corrupt");
        let _ = std::fs::remove_file(&corrupt);
        let _ = std::fs::rename(&self.file_path, &corrupt);
    }
}

impl SettingsPersistence for SettingsStore {
    /// Reads settings from disk and normalizes them. A missing, unreadable, or
    /// corrupt file falls back to defaults; a corrupt one is first moved aside
    /// to `settings.json.corrupt` so the next save does not destroy it.
    fn load(&self) -> AppSettings {
        self.flush();
        let data = match std::fs::read(&self.file_path) {
            Ok(data) => data,
            Err(_) => return AppSettings::default(),
        };
        match serde_json::from_slice::<AppSettings>(&data) {
            Ok(settings) => settings.normalized(),
            Err(_) => {
                self.preserve_corrupt_file();
                AppSettings::default()
            }
        }
    }

    /// Persists settings, normalizing them first so out-of-range values never
    /// reach disk.
    fn save(&self, settings: &AppSettings) {
        if let Some(writer) = &self.writer {
            let _ = writer.send(WriteRequest::Save(settings.normalized()));
        } else {
            let _ = write_settings(&self.file_path, settings);
        }
    }

    fn flush(&self) {
        if let Some(writer) = &self.writer {
            let (reply, done) = mpsc::channel();
            if writer.send(WriteRequest::Flush(reply)).is_ok() {
                let _ = done.recv_timeout(Duration::from_secs(2));
            }
        }
    }
}

impl Drop for SettingsStore {
    fn drop(&mut self) {
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonglow_core::TimeOfDay;

    fn temp_store(name: &str) -> (SettingsStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "moonglow-test-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("settings.json");
        (SettingsStore::new(path.clone()), dir)
    }

    #[test]
    fn missing_file_loads_defaults() {
        let (store, _dir) = temp_store("missing");
        assert_eq!(store.load(), AppSettings::default());
    }

    #[test]
    fn dropping_the_store_flushes_the_latest_pending_value() {
        let (store, dir) = temp_store("flush-before-exit");
        let path = store.file_path.clone();
        for manual_brightness in 20..=60 {
            store.save(&AppSettings {
                manual_brightness,
                ..AppSettings::default()
            });
        }
        drop(store);
        let restarted = SettingsStore::new(path);
        assert_eq!(restarted.load().manual_brightness, 60);
        drop(restarted);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_loads_defaults() {
        let (store, dir) = temp_store("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&store.file_path, "{ not json").unwrap();
        assert_eq!(store.load(), AppSettings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_preserved_before_falling_back() {
        let (store, dir) = temp_store("corrupt-preserve");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&store.file_path, "{ not json").unwrap();
        assert_eq!(store.load(), AppSettings::default());
        let preserved = store.file_path.with_extension("json.corrupt");
        assert!(preserved.exists());
        assert_eq!(std::fs::read_to_string(&preserved).unwrap(), "{ not json");
        assert!(!store.file_path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trips_and_normalizes() {
        let (store, dir) = temp_store("roundtrip");
        let settings = AppSettings {
            manual_brightness: 42,
            morning_start: TimeOfDay::new(5, 15),
            manual_color_temp_k: 99999, // normalized on save
            ..AppSettings::default()
        };
        store.save(&settings);
        let loaded = store.load();
        assert_eq!(loaded.manual_brightness, 42);
        assert_eq!(loaded.morning_start, TimeOfDay::new(5, 15));
        assert_eq!(loaded.manual_color_temp_k, 6600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
