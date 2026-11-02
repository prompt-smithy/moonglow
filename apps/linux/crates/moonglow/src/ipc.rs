//! Single-instance enforcement and remote control over a Unix socket.
//!
//! Protocol: newline-delimited JSON. Requests look like
//! `{"cmd":"set_brightness","value":80}`; responses are `{"ok":true,...}` or
//! `{"error":"..."}`. A second invocation forwards its command to the running
//! instance instead of starting a second schedule loop and window.

use crate::commands::{AppCommand, SharedState};
use moonglow_core::state::{ScheduleTime, ScheduleWindow};
use moonglow_core::time_of_day::TimeOfDay;
use moonglow_core::traits::SettingsPersistence;
use std::io::{BufRead, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// `$XDG_RUNTIME_DIR/moonglow.sock`, falling back to a
/// per-user temp path.
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("moonglow.sock");
        }
    }
    let uid = rustix_user();
    std::env::temp_dir().join(format!("moonglow-{uid}.sock"))
}

fn rustix_user() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Held before probing or changing hardware, including during one-shot commands.
/// The lock file stays in place: unlinking it could create two independent locks.
pub struct InstanceLock {
    _file: std::fs::File,
}

impl InstanceLock {
    pub fn try_acquire() -> std::io::Result<Option<Self>> {
        Self::at_path(&socket_path().with_extension("lock"))
    }

    fn at_path(path: &std::path::Path) -> std::io::Result<Option<Self>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

/// Whether a live instance answers at the socket.
pub fn ping() -> bool {
    matches!(
        send(&serde_json::json!({"cmd": "ping"})),
        Ok(response) if response.get("ok").and_then(|v| v.as_bool()) == Some(true)
    )
}

/// Sends one command to the running instance.
pub fn send(cmd: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut stream =
        UnixStream::connect(socket_path()).map_err(|e| format!("no running instance ({e})"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut line = serde_json::to_string(cmd).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut reader = std::io::BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).map_err(|e| e.to_string())?;
    let response: serde_json::Value =
        serde_json::from_str(&response).map_err(|e| format!("bad response: {e}"))?;
    if let Some(error) = response.get("error").and_then(|v| v.as_str()) {
        return Err(error.to_string());
    }
    Ok(response)
}

/// Handle to the running IPC server thread.
pub struct IpcHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl IpcHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // The socket file is deliberately left behind: removing it here could
        // unlink a successor that already rebound the path. A stale file is
        // harmless — the next starter takes it over in `bind_socket`.
    }
}

/// Whether something still listens at the socket path.
fn socket_owner_alive(path: &std::path::Path) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}

/// Binds the socket, taking over only a stale file. A live owner is never
/// unlinked, so two primaries can never run side by side (a racing loser
/// re-checks liveness instead of stealing the winner's socket).
fn bind_socket(path: &std::path::Path) -> std::io::Result<UnixListener> {
    match UnixListener::bind(path) {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if socket_owner_alive(path) {
                Err(e)
            } else {
                let _ = std::fs::remove_file(path);
                UnixListener::bind(path)
            }
        }
        Err(e) => Err(e),
    }
}

/// Serves the socket on a background thread. Callers must have checked
/// [`ping`]; a racing loser fails here and exits instead of stealing the
/// winner's socket.
pub fn spawn_ipc<S: SettingsPersistence + 'static>(
    state: SharedState<S>,
    cmd_tx: mpsc::Sender<AppCommand>,
) -> Result<IpcHandle, String> {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let listener =
        bind_socket(&path).map_err(|e| format!("cannot bind {}: {e}", path.display()))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("moonglow-ipc".to_string())
        .spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle_connection(stream, &state, &cmd_tx),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("ipc thread failed: {e}"))?;
    Ok(IpcHandle {
        stop,
        thread: Some(thread),
    })
}

fn handle_connection<S: SettingsPersistence>(
    stream: UnixStream,
    state: &SharedState<S>,
    cmd_tx: &mpsc::Sender<AppCommand>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = std::io::BufReader::new(stream.try_clone().unwrap_or(stream));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let request: serde_json::Value = serde_json::from_str(&line).unwrap_or(serde_json::json!({}));
    let response = dispatch(&request, state, cmd_tx);
    let mut line =
        serde_json::to_string(&response).unwrap_or_else(|_| r#"{"error":"encode"}"#.to_string());
    line.push('\n');
    let _ = reader.get_mut().write_all(line.as_bytes());
}

fn dispatch<S: SettingsPersistence>(
    request: &serde_json::Value,
    state: &SharedState<S>,
    cmd_tx: &mpsc::Sender<AppCommand>,
) -> serde_json::Value {
    let cmd = request.get("cmd").and_then(|c| c.as_str()).unwrap_or("");
    match cmd {
        "ping" => serde_json::json!({"ok": true}),
        "show_settings" => {
            let _ = cmd_tx.send(AppCommand::ShowWindow);
            crate::settings_ui::wake_ui();
            serde_json::json!({"ok": true})
        }
        "quit" => {
            // Direct first (never waits for the event loop), channel as backup.
            crate::request_quit();
            let _ = cmd_tx.send(AppCommand::Quit);
            crate::settings_ui::wake_ui();
            serde_json::json!({"ok": true})
        }
        "set_brightness" => match request.get("value").and_then(|v| v.as_i64()) {
            Some(value) => match state.lock() {
                Ok(mut s) => {
                    s.set_brightness(value as i32);
                    serde_json::json!({"ok": true, "brightness": s.brightness()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'value'"}),
        },
        "adjust_brightness" => match request.get("delta").and_then(|v| v.as_i64()) {
            Some(delta) => match state.lock() {
                Ok(mut s) => {
                    let next = s.brightness() + delta as i32;
                    s.set_brightness(next);
                    serde_json::json!({"ok": true, "brightness": s.brightness()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'delta'"}),
        },
        "hotkey" => match request.get("action").and_then(|v| v.as_str()) {
            Some(action) => match state.lock() {
                Ok(mut s) => {
                    match action {
                        "increase-brightness" => s.increase_brightness(),
                        "decrease-brightness" => s.decrease_brightness(),
                        "increase-temperature" => s.increase_color_temperature(),
                        "decrease-temperature" => s.decrease_color_temperature(),
                        _ => {
                            return serde_json::json!({"error": format!("unknown hotkey action '{action}'")})
                        }
                    }
                    serde_json::json!({
                        "ok": true,
                        "brightness": s.brightness(),
                        "temperature": s.color_temperature(),
                        "auto": s.auto_color_temperature(),
                    })
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing string 'action'"}),
        },
        "adjust_temp" => match request.get("delta").and_then(|v| v.as_i64()) {
            Some(delta) => match state.lock() {
                Ok(mut s) => {
                    let next = s.color_temperature() + delta as i32;
                    s.set_color_temperature(next);
                    serde_json::json!({"ok": true, "temperature": s.color_temperature()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'delta'"}),
        },
        "set_temp" => match request.get("kelvin").and_then(|v| v.as_i64()) {
            Some(kelvin) => match state.lock() {
                Ok(mut s) => {
                    s.set_color_temperature(kelvin as i32);
                    serde_json::json!({"ok": true, "temperature": s.color_temperature()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'kelvin'"}),
        },
        "set_auto" => match request.get("on").and_then(|v| v.as_bool()) {
            Some(on) => match state.lock() {
                Ok(mut s) => {
                    s.set_auto_color_temperature(on);
                    serde_json::json!({"ok": true, "auto": s.auto_color_temperature()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing boolean 'on'"}),
        },
        "set_day_temp" => match request.get("kelvin").and_then(|v| v.as_i64()) {
            Some(kelvin) => match state.lock() {
                Ok(mut s) => {
                    s.set_day_color_temp(kelvin as i32);
                    serde_json::json!({"ok": true, "dayTemp": s.day_color_temp()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'kelvin'"}),
        },
        "set_night_temp" => match request.get("kelvin").and_then(|v| v.as_i64()) {
            Some(kelvin) => match state.lock() {
                Ok(mut s) => {
                    s.set_night_color_temp(kelvin as i32);
                    serde_json::json!({"ok": true, "nightTemp": s.night_color_temp()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing integer 'kelvin'"}),
        },
        "set_schedule_window" => {
            let window = match request.get("window").and_then(|v| v.as_str()) {
                Some("morning") => ScheduleWindow::Morning,
                Some("evening") => ScheduleWindow::Evening,
                _ => return serde_json::json!({"error": "expected window=morning|evening"}),
            };
            let parse = |key| {
                request
                    .get(key)
                    .and_then(|v| v.as_str())
                    .and_then(|raw| raw.parse::<TimeOfDay>().ok())
            };
            let (Some(from), Some(to)) = (parse("from"), parse("to")) else {
                return serde_json::json!({"error": "expected from/to in HH:MM format"});
            };
            if from >= to {
                return serde_json::json!({"error": "transition start must precede its end"});
            }
            match state.lock() {
                Ok(mut s) => {
                    s.set_schedule_window(window, from, to);
                    let (start, end) = match window {
                        ScheduleWindow::Morning => {
                            (ScheduleTime::MorningStart, ScheduleTime::MorningEnd)
                        }
                        ScheduleWindow::Evening => {
                            (ScheduleTime::EveningStart, ScheduleTime::EveningEnd)
                        }
                    };
                    serde_json::json!({"ok": true, "from": s.schedule_time(start).to_string(), "to": s.schedule_time(end).to_string()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            }
        }
        "set_schedule" => {
            let field = request.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let which = match field {
                "morning_start" => Some(ScheduleTime::MorningStart),
                "morning_end" => Some(ScheduleTime::MorningEnd),
                "evening_start" => Some(ScheduleTime::EveningStart),
                "evening_end" => Some(ScheduleTime::EveningEnd),
                _ => None,
            };
            let value = request
                .get("value")
                .and_then(|v| v.as_str())
                .and_then(|raw| raw.parse::<TimeOfDay>().ok());
            match (which, value) {
                (Some(which), Some(value)) => match state.lock() {
                    Ok(mut s) => {
                        s.set_schedule_time(which, value);
                        serde_json::json!({
                            "ok": true,
                            "field": field,
                            "value": s.schedule_time(which).to_string(),
                        })
                    }
                    Err(_) => serde_json::json!({"error": "state locked"}),
                },
                _ => serde_json::json!({
                    "error": "expected field=morning_start|morning_end|evening_start|evening_end and value=\"HH:MM\"",
                }),
            }
        }
        "set_login" => match request.get("on").and_then(|v| v.as_bool()) {
            Some(on) => match state.lock() {
                Ok(mut s) => {
                    s.set_launch_at_login(on);
                    serde_json::json!({"ok": true, "login": s.launch_at_login()})
                }
                Err(_) => serde_json::json!({"error": "state locked"}),
            },
            None => serde_json::json!({"error": "missing boolean 'on'"}),
        },
        "status" => match state.lock() {
            Ok(s) => serde_json::json!({
                "ok": true,
                "brightness": s.brightness(),
                "temperature": s.color_temperature(),
                "auto": s.auto_color_temperature(),
                "dayTemp": s.day_color_temp(),
                "nightTemp": s.night_color_temp(),
                "schedule": {
                    "morningStart": s.schedule_time(ScheduleTime::MorningStart).to_string(),
                    "morningEnd": s.schedule_time(ScheduleTime::MorningEnd).to_string(),
                    "eveningStart": s.schedule_time(ScheduleTime::EveningStart).to_string(),
                    "eveningEnd": s.schedule_time(ScheduleTime::EveningEnd).to_string(),
                },
                "login": s.launch_at_login(),
                "hasBrightness": s.has_brightness_support(),
                "hasColor": s.has_color_support(),
                "brightnessStatus": s.brightness_status(),
                "colorStatus": s.color_status(),
                "displays": s.displays().iter().map(|d| serde_json::json!({
                    "name": d.name,
                    "primary": d.is_primary,
                    "summary": d.capability_summary(),
                })).collect::<Vec<_>>(),
            }),
            Err(_) => serde_json::json!({"error": "state locked"}),
        },
        _ => serde_json::json!({"error": format!("unknown command '{cmd}'")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonglow_core::settings::AppSettings;
    use moonglow_core::state::StateManager;
    use moonglow_core::traits::{
        BrightnessService, ColorTemperatureService, DisplayService, LoginItemService,
        SettingsPersistence,
    };
    use moonglow_core::DisplayInfo;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemStore(Mutex<AppSettings>);

    impl SettingsPersistence for MemStore {
        fn load(&self) -> AppSettings {
            self.0.lock().unwrap().clone()
        }
        fn save(&self, settings: &AppSettings) {
            *self.0.lock().unwrap() = settings.clone();
        }
    }

    struct StubDisplay;
    impl DisplayService for StubDisplay {
        fn displays(&self) -> Vec<DisplayInfo> {
            vec![DisplayInfo::new("1", "Test", "1", true, true, true)]
        }
        fn refresh(&mut self) -> Vec<DisplayInfo> {
            self.displays()
        }
    }

    struct StubBrightness;
    impl BrightnessService for StubBrightness {
        fn is_supported(&self) -> bool {
            true
        }
        fn set_brightness(&self, _percent: i32) {}
    }

    struct StubColor;
    impl ColorTemperatureService for StubColor {
        fn is_supported(&self) -> bool {
            true
        }
        fn apply(&self, _kelvin: i32) -> bool {
            true
        }
        fn reset(&self) {}
    }

    struct StubLogin;
    impl LoginItemService for StubLogin {
        fn is_enabled(&self) -> bool {
            false
        }
        fn requires_approval(&self) -> bool {
            false
        }
        fn set_enabled(&mut self, _enabled: bool) -> bool {
            true
        }
    }

    fn test_state() -> SharedState<MemStore> {
        Arc::new(Mutex::new(StateManager::new(
            MemStore::default(),
            Box::new(StubDisplay),
            Arc::new(Mutex::new(StubBrightness)),
            Arc::new(Mutex::new(StubColor)),
            Box::new(StubLogin),
        )))
    }

    #[test]
    fn dispatches_known_commands() {
        let state = test_state();
        let (tx, rx) = mpsc::channel();
        let status = dispatch(&serde_json::json!({"cmd": "status"}), &state, &tx);
        assert_eq!(status["ok"], true);
        assert_eq!(status["brightness"], 80);

        let set = dispatch(
            &serde_json::json!({"cmd": "set_brightness", "value": 42}),
            &state,
            &tx,
        );
        assert_eq!(set["brightness"], 42);

        let bad = dispatch(&serde_json::json!({"cmd": "set_brightness"}), &state, &tx);
        assert!(bad.get("error").is_some());

        let up = dispatch(
            &serde_json::json!({"cmd": "adjust_brightness", "delta": 5}),
            &state,
            &tx,
        );
        assert_eq!(up["brightness"], 47);
        let down = dispatch(
            &serde_json::json!({"cmd": "adjust_temp", "delta": -100}),
            &state,
            &tx,
        );
        assert!(down["temperature"].as_i64().unwrap() < 6500);

        let unknown = dispatch(&serde_json::json!({"cmd": "frobnicate"}), &state, &tx);
        assert!(unknown.get("error").is_some());

        dispatch(&serde_json::json!({"cmd": "quit"}), &state, &tx);
        assert_eq!(rx.try_recv().unwrap(), AppCommand::Quit);
    }

    #[test]
    fn dispatches_settings_commands() {
        let state = test_state();
        let (tx, _rx) = mpsc::channel();
        let day = dispatch(
            &serde_json::json!({"cmd": "set_day_temp", "kelvin": 6000}),
            &state,
            &tx,
        );
        assert_eq!(day["dayTemp"], 6000);
        let night = dispatch(
            &serde_json::json!({"cmd": "set_night_temp", "kelvin": 2700}),
            &state,
            &tx,
        );
        assert_eq!(night["nightTemp"], 2700);
        let missing = dispatch(&serde_json::json!({"cmd": "set_day_temp"}), &state, &tx);
        assert!(missing.get("error").is_some());

        let window = dispatch(
            &serde_json::json!({
                "cmd": "set_schedule",
                "field": "morning_start",
                "value": "06:30",
            }),
            &state,
            &tx,
        );
        assert_eq!(window["ok"], true);
        assert_eq!(window["value"], "06:30");
        let bad_field = dispatch(
            &serde_json::json!({
                "cmd": "set_schedule",
                "field": "midnight_snack",
                "value": "06:30",
            }),
            &state,
            &tx,
        );
        assert!(bad_field.get("error").is_some());
        let bad_time = dispatch(
            &serde_json::json!({
                "cmd": "set_schedule",
                "field": "morning_start",
                "value": "25:99",
            }),
            &state,
            &tx,
        );
        assert!(bad_time.get("error").is_some());

        let login = dispatch(
            &serde_json::json!({"cmd": "set_login", "on": true}),
            &state,
            &tx,
        );
        assert_eq!(login["login"], true);

        let status = dispatch(&serde_json::json!({"cmd": "status"}), &state, &tx);
        assert_eq!(status["dayTemp"], 6000);
        assert_eq!(status["nightTemp"], 2700);
        assert_eq!(status["schedule"]["morningStart"], "06:30");
        assert_eq!(status["login"], true);
    }

    #[test]
    fn live_socket_is_never_stolen() {
        let path = std::env::temp_dir().join(format!("moonglow-bind-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let owner = UnixListener::bind(&path).unwrap();
        // A live owner answers: takeover refused, so no second primary.
        assert!(bind_socket(&path).is_err());
        drop(owner);
        // Nobody listens: the stale file is taken over.
        let taken = bind_socket(&path);
        assert!(taken.is_ok());
        drop(taken);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn socket_path_is_per_user() {
        let path = socket_path();
        assert!(path.to_string_lossy().contains("moonglow"));
    }

    #[test]
    fn instance_lock_excludes_a_racing_start_before_socket_exists() {
        let path = std::env::temp_dir().join(format!("moonglow-lock-test-{}", std::process::id()));
        let first = InstanceLock::at_path(&path).unwrap().unwrap();
        assert!(InstanceLock::at_path(&path).unwrap().is_none());
        drop(first);
        assert!(InstanceLock::at_path(&path).unwrap().is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn schedule_window_moves_atomically_and_rejects_invalid_pairs() {
        let state = test_state();
        let (tx, _rx) = mpsc::channel();
        let result = dispatch(
            &serde_json::json!({"cmd":"set_schedule_window", "window":"morning", "from":"08:00", "to":"09:00"}),
            &state,
            &tx,
        );
        assert_eq!(result["from"], "08:00");
        assert_eq!(result["to"], "09:00");
        let invalid = dispatch(
            &serde_json::json!({"cmd":"set_schedule_window", "window":"morning", "from":"10:00", "to":"07:00"}),
            &state,
            &tx,
        );
        assert!(invalid.get("error").is_some());
        assert_eq!(
            state
                .lock()
                .unwrap()
                .schedule_time(ScheduleTime::MorningStart),
            TimeOfDay::new(8, 0)
        );
    }
}
