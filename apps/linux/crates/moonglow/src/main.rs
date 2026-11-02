//! Moonglow for Linux.
//!
//! Desktop application for external-display DDC/CI brightness and scheduled
//! colour temperature. This module is the composition root (deliberately
//! hand-rolled, like the macOS `AppDelegate`): it picks the gamma backend for
//! the running session, wires the shared state, and owns shutdown — including
//! restoring the pre-launch gamma state so quitting never leaves a display
//! tinted.

mod autostart;
mod commands;
mod ddc;
mod displays;
mod gnome;
mod ipc;
mod kde;
mod logging;
mod runtime;
mod session;
mod settings_ui;
mod shortcuts;
mod store;
mod visibility;
mod wlr;
mod x11;

use clap::Parser;
use commands::{AppCommand, SharedState};
use moonglow_core::schedule::TICK_INTERVAL_SECS;
use moonglow_core::settings::KELVIN_STEP;
use moonglow_core::state::{ScheduleTime, ScheduleWindow, StateManager};
use moonglow_core::time_of_day::TimeOfDay;
use moonglow_core::traits::{
    BrightnessFallbackService, BrightnessService, ColorTemperatureService, DisplayService,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "moonglow",
    version,
    about = "External-display brightness, scheduled colour temperature, and global shortcuts"
)]
struct Cli {
    /// Show the settings window (forwards to the running instance).
    #[arg(long)]
    show_settings: bool,
    /// Quit the running instance.
    #[arg(long)]
    quit: bool,
    /// Set brightness 1...100.
    #[arg(long, value_name = "PERCENT")]
    set_brightness: Option<i32>,
    /// Raise brightness by 5%.
    #[arg(long)]
    brighter: bool,
    /// Lower brightness by 5%.
    #[arg(long)]
    dimmer: bool,
    /// Set colour temperature in kelvin (manual mode).
    #[arg(long, value_name = "KELVIN")]
    set_temp: Option<i32>,
    /// Warm the temperature by 100 K (manual mode).
    #[arg(long)]
    warmer: bool,
    /// Cool the temperature by 100 K (manual mode).
    #[arg(long)]
    cooler: bool,
    /// Turn automatic temperature on or off.
    #[arg(long, value_name = "on|off")]
    auto: Option<String>,
    /// Set the daytime schedule target in kelvin.
    #[arg(long, value_name = "KELVIN")]
    day_temp: Option<i32>,
    /// Set the nighttime schedule target in kelvin.
    #[arg(long, value_name = "KELVIN")]
    night_temp: Option<i32>,
    /// Set the morning transition window (24h "HH:MM").
    #[arg(long, value_names = ["FROM", "TO"], num_args = 2)]
    morning: Option<Vec<String>>,
    /// Set the evening transition window (24h "HH:MM").
    #[arg(long, value_names = ["FROM", "TO"], num_args = 2)]
    evening: Option<Vec<String>>,
    /// Turn start-at-login on or off.
    #[arg(long, value_name = "on|off")]
    login: Option<String>,
    /// Print status and exit.
    #[arg(long)]
    status: bool,
    /// With --status: print machine-readable JSON.
    #[arg(long)]
    json: bool,
    /// Start with the window hidden (still running in the background).
    #[arg(long)]
    hidden: bool,
    /// Run without the settings window (schedule and shortcuts only).
    #[arg(long)]
    headless: bool,
    /// Global-shortcut action: raise brightness by 5%.
    #[arg(long)]
    brightness_up: bool,
    /// Global-shortcut action: lower brightness by 5%.
    #[arg(long)]
    brightness_down: bool,
    /// Global-shortcut action: raise colour temperature by 100 K.
    #[arg(long)]
    temperature_up: bool,
    /// Global-shortcut action: lower colour temperature by 100 K.
    #[arg(long)]
    temperature_down: bool,
}

/// Forwards `ColorTemperatureService` to a shared concrete backend so the
/// colour service and the software-brightness service drive the same ramps.
struct SharedColor<T: ColorTemperatureService + Send>(Arc<Mutex<T>>);

impl<T: ColorTemperatureService + Send> ColorTemperatureService for SharedColor<T> {
    fn backend_name(&self) -> String {
        self.0.lock().map(|c| c.backend_name()).unwrap_or_default()
    }

    fn is_supported(&self) -> bool {
        self.0.lock().map(|c| c.is_supported()).unwrap_or(false)
    }

    fn apply(&self, kelvin: i32) -> bool {
        self.0.lock().map(|c| c.apply(kelvin)).unwrap_or(false)
    }

    fn reset(&self) {
        if let Ok(c) = self.0.lock() {
            c.reset();
        }
    }
}

/// Brightness backend used when no software dimming exists (GNOME/KDE night
/// lights have no brightness channel): always unsupported, so the fallback
/// resolves to DDC/CI or nothing.
struct UnsupportedBrightness;

impl BrightnessService for UnsupportedBrightness {
    fn is_supported(&self) -> bool {
        false
    }

    fn set_brightness(&self, _percent: i32) {}
}

struct Composed {
    display: Box<dyn DisplayService>,
    hardware: Arc<Mutex<ddc::LinuxDdcService>>,
    color: Arc<Mutex<dyn ColorTemperatureService>>,
    brightness: BrightnessFallbackService,
}

/// Probes hardware and picks the gamma backend for the running session.
/// Never fails: with no display at all the app still runs and reports every
/// capability as unavailable.
fn compose_backends() -> Composed {
    let hardware = Arc::new(Mutex::new(ddc::LinuxDdcService::probe()));
    let ddc_supported = hardware.lock().map(|d| d.is_supported()).unwrap_or(false);
    if ddc_supported {
        eprintln!("brightness: DDC/CI panel found");
    }

    let session_kind = session::detect_session();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    eprintln!("session: {session_kind:?} (desktop: {desktop})");

    let try_x11 = || {
        let gamma = Arc::new(Mutex::new(x11::X11GammaService::connect().ok()?));
        let software: Arc<Mutex<dyn BrightnessService>> = Arc::new(Mutex::new(
            x11::X11SoftwareBrightnessService::new(gamma.clone()),
        ));
        let color: Arc<Mutex<dyn ColorTemperatureService>> =
            Arc::new(Mutex::new(SharedColor(gamma)));
        Some((color, software, "X11 gamma control"))
    };
    let try_wlr = || {
        let gamma = Arc::new(Mutex::new(wlr::WlrGammaService::probe().ok()?));
        let software: Arc<Mutex<dyn BrightnessService>> = Arc::new(Mutex::new(
            wlr::WlrSoftwareBrightnessService::new(gamma.clone()),
        ));
        let color: Arc<Mutex<dyn ColorTemperatureService>> =
            Arc::new(Mutex::new(SharedColor(gamma)));
        Some((color, software, "wlroots gamma control"))
    };
    let try_gnome = || {
        let backend = gnome::GnomeNightLightService::probe().ok()?;
        let color: Arc<Mutex<dyn ColorTemperatureService>> = Arc::new(Mutex::new(backend));
        let software: Arc<Mutex<dyn BrightnessService>> =
            Arc::new(Mutex::new(UnsupportedBrightness));
        Some((color, software, "GNOME Night Light control"))
    };
    let try_kde = || {
        let backend = kde::KwinNightLightService::probe().ok()?;
        let color: Arc<Mutex<dyn ColorTemperatureService>> = Arc::new(Mutex::new(backend));
        let software: Arc<Mutex<dyn BrightnessService>> =
            Arc::new(Mutex::new(UnsupportedBrightness));
        Some((color, software, "KDE Night Light control"))
    };
    let desktop_lower = desktop.to_lowercase();
    let desktop_native = || {
        if desktop_lower.contains("gnome") {
            try_gnome()
        } else if desktop_lower.contains("kde") || desktop_lower.contains("plasma") {
            try_kde()
        } else {
            None
        }
    };

    // Each candidate probes for real; order is preference, probing decides.
    let found = match session_kind {
        session::SessionKind::X11 => try_x11(),
        session::SessionKind::XWayland => try_x11().or_else(desktop_native).or_else(try_wlr),
        session::SessionKind::GnomeWayland => try_gnome().or_else(try_wlr),
        session::SessionKind::KdeWayland => try_kde().or_else(try_wlr),
        session::SessionKind::WlrWayland | session::SessionKind::UnknownWayland => {
            try_wlr().or_else(desktop_native)
        }
        session::SessionKind::Headless => try_x11().or_else(try_wlr),
    };
    let (color, software, backend_name) = match found {
        Some(found) => {
            eprintln!("colour: {}", found.2);
            found
        }
        None => {
            eprintln!("colour: no supported backend");
            let color: Arc<Mutex<dyn ColorTemperatureService>> =
                Arc::new(Mutex::new(UnsupportedColor));
            let software: Arc<Mutex<dyn BrightnessService>> =
                Arc::new(Mutex::new(UnsupportedBrightness));
            (color, software, "none")
        }
    };
    let _ = backend_name;

    let display_hardware = hardware.clone();
    let display_software = software.clone();
    let display_color = color.clone();
    let display: Box<dyn DisplayService> = match session_kind {
        session::SessionKind::X11 | session::SessionKind::XWayland => {
            Box::new(x11::X11DisplayService::new(move || {
                display_hardware
                    .lock()
                    .map(|h| h.is_supported())
                    .unwrap_or(false)
            }))
        }
        _ => Box::new(displays::SysfsDisplayService::with_live_capabilities(
            move || {
                (
                    display_hardware
                        .lock()
                        .map(|h| h.is_supported())
                        .unwrap_or(false),
                    display_software
                        .lock()
                        .map(|s| s.is_supported())
                        .unwrap_or(false),
                    display_color
                        .lock()
                        .map(|c| c.is_supported())
                        .unwrap_or(false),
                )
            },
        )),
    };

    let brightness = BrightnessFallbackService::new(hardware.clone(), software);
    Composed {
        display,
        hardware,
        color,
        brightness,
    }
}

struct UnsupportedColor;

impl ColorTemperatureService for UnsupportedColor {
    fn backend_name(&self) -> String {
        "No colour control".to_string()
    }

    fn is_supported(&self) -> bool {
        false
    }

    fn apply(&self, _kelvin: i32) -> bool {
        false
    }

    fn reset(&self) {}
}

/// Whether this is the first launch of the installed version. Preserves the
/// macOS install-id semantics (reset autostart on fresh install) through a
/// version marker file.
fn is_first_launch_after_install() -> bool {
    let marker = store::default_file_path()
        .parent()
        .map(|dir| dir.join("install-id"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/moonglow-install-id"));
    let current = env!("CARGO_PKG_VERSION");
    let previous = std::fs::read_to_string(&marker).unwrap_or_default();
    if previous.trim() == current {
        return false;
    }
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&marker, current);
    true
}

/// Parses a FROM/TO "HH:MM" pair. Both are validated before anything is
/// applied so a bad pair never half-applies.
fn parse_window(flag: &str, pair: &[String]) -> Result<(TimeOfDay, TimeOfDay), i32> {
    let [from, to] = pair else {
        eprintln!("{flag} expects FROM and TO times.");
        return Err(2);
    };
    let from = from.parse::<TimeOfDay>();
    let to = to.parse::<TimeOfDay>();
    match (from, to) {
        (Ok(from), Ok(to)) if from < to => Ok((from, to)),
        _ => {
            eprintln!("{flag} expects 24h \"HH:MM\" times.");
            Err(2)
        }
    }
}

fn print_status(status: &serde_json::Value) {
    println!(
        "brightness:   {}% ({})",
        status["brightness"],
        status["brightnessStatus"].as_str().unwrap_or("")
    );
    println!(
        "temperature:  {} K ({})",
        status["temperature"],
        if status["auto"].as_bool().unwrap_or(false) {
            "automatic"
        } else {
            "manual"
        }
    );
    println!(
        "colour:       {}",
        status["colorStatus"].as_str().unwrap_or("")
    );
    println!(
        "schedule:     day {} K · night {} K · morning {}-{} · evening {}-{}",
        status["dayTemp"],
        status["nightTemp"],
        status["schedule"]["morningStart"].as_str().unwrap_or("?"),
        status["schedule"]["morningEnd"].as_str().unwrap_or("?"),
        status["schedule"]["eveningStart"].as_str().unwrap_or("?"),
        status["schedule"]["eveningEnd"].as_str().unwrap_or("?"),
    );
    println!(
        "start login:  {}",
        if status["login"].as_bool().unwrap_or(false) {
            "on"
        } else {
            "off"
        }
    );
    println!("displays:");
    if let Some(displays) = status["displays"].as_array() {
        if displays.is_empty() {
            println!("  (none detected)");
        }
        for display in displays {
            println!(
                "  {}: {}",
                display["name"].as_str().unwrap_or("?"),
                display["summary"].as_str().unwrap_or("")
            );
        }
    }
}

/// One-shot control without a running daemon: drives the backends directly,
/// persists, and exits. Useful headless and in scripts.
fn direct_command(cli: &Cli) -> i32 {
    if cli.quit {
        println!("moonglow is not running.");
        return 0;
    }
    let composed = compose_backends();
    let store = store::SettingsStore::default_path();
    let login = autostart::AutostartService::default_path();
    let mut state = StateManager::new(
        store,
        composed.display,
        Arc::new(Mutex::new(composed.brightness)),
        composed.color.clone(),
        Box::new(login),
    );
    if let Some(percent) = cli.set_brightness {
        state.set_brightness(percent);
        println!("brightness: {}%", state.brightness());
    }
    if cli.brighter {
        state.set_brightness(state.brightness() + 5);
        println!("brightness: {}%", state.brightness());
    }
    if cli.dimmer {
        state.set_brightness(state.brightness() - 5);
        println!("brightness: {}%", state.brightness());
    }
    if cli.brightness_up {
        state.increase_brightness();
        println!("brightness: {}%", state.brightness());
    }
    if cli.brightness_down {
        state.decrease_brightness();
        println!("brightness: {}%", state.brightness());
    }
    if cli.temperature_up {
        state.increase_color_temperature();
        println!("temperature: {} K", state.color_temperature());
        println!("automatic: {}", state.auto_color_temperature());
    }
    if cli.temperature_down {
        state.decrease_color_temperature();
        println!("temperature: {} K", state.color_temperature());
        println!("automatic: {}", state.auto_color_temperature());
    }
    if let Some(kelvin) = cli.set_temp {
        if state.auto_color_temperature() {
            println!(
                "note: automatic mode is on; the schedule will override this on the next tick."
            );
        }
        state.set_color_temperature(kelvin);
        println!("temperature: {} K", state.color_temperature());
    }
    if cli.warmer || cli.cooler {
        if state.auto_color_temperature() {
            println!(
                "note: automatic mode is on; the schedule will override this on the next tick."
            );
        }
        let delta = if cli.warmer {
            -KELVIN_STEP
        } else {
            KELVIN_STEP
        };
        state.set_color_temperature(state.color_temperature() + delta);
        println!("temperature: {} K", state.color_temperature());
    }
    if let Some(auto) = &cli.auto {
        match auto.as_str() {
            "on" => {
                state.set_auto_color_temperature(true);
                println!("automatic: on");
            }
            "off" => {
                state.set_auto_color_temperature(false);
                println!("automatic: off");
            }
            _ => {
                eprintln!("--auto expects 'on' or 'off'.");
                return 2;
            }
        }
    }
    if let Some(kelvin) = cli.day_temp {
        state.set_day_color_temp(kelvin);
        println!("day temperature: {} K", state.day_color_temp());
    }
    if let Some(kelvin) = cli.night_temp {
        state.set_night_color_temp(kelvin);
        println!("night temperature: {} K", state.night_color_temp());
    }
    if let Some(pair) = &cli.morning {
        let (from, to) = match parse_window("--morning", pair) {
            Ok(times) => times,
            Err(code) => return code,
        };
        state.set_schedule_window(ScheduleWindow::Morning, from, to);
        println!(
            "morning: {} - {}",
            state.schedule_time(ScheduleTime::MorningStart),
            state.schedule_time(ScheduleTime::MorningEnd)
        );
    }
    if let Some(pair) = &cli.evening {
        let (from, to) = match parse_window("--evening", pair) {
            Ok(times) => times,
            Err(code) => return code,
        };
        state.set_schedule_window(ScheduleWindow::Evening, from, to);
        println!(
            "evening: {} - {}",
            state.schedule_time(ScheduleTime::EveningStart),
            state.schedule_time(ScheduleTime::EveningEnd)
        );
    }
    if let Some(login) = &cli.login {
        match login.as_str() {
            "on" => {
                state.set_launch_at_login(true);
                println!("start at login: {}", state.launch_at_login());
            }
            "off" => {
                state.set_launch_at_login(false);
                println!("start at login: {}", state.launch_at_login());
            }
            _ => {
                eprintln!("--login expects 'on' or 'off'.");
                return 2;
            }
        }
    }
    if cli.status
        || (cli.set_brightness.is_none()
            && cli.set_temp.is_none()
            && cli.auto.is_none()
            && cli.day_temp.is_none()
            && cli.night_temp.is_none()
            && cli.morning.is_none()
            && cli.evening.is_none()
            && cli.login.is_none()
            && !cli.brighter
            && !cli.dimmer
            && !cli.warmer
            && !cli.cooler
            && !cli.brightness_up
            && !cli.brightness_down
            && !cli.temperature_up
            && !cli.temperature_down)
    {
        let status = serde_json::json!({
            "brightness": state.brightness(),
            "temperature": state.color_temperature(),
            "auto": state.auto_color_temperature(),
            "dayTemp": state.day_color_temp(),
            "nightTemp": state.night_color_temp(),
            "schedule": {
                "morningStart": state.schedule_time(ScheduleTime::MorningStart).to_string(),
                "morningEnd": state.schedule_time(ScheduleTime::MorningEnd).to_string(),
                "eveningStart": state.schedule_time(ScheduleTime::EveningStart).to_string(),
                "eveningEnd": state.schedule_time(ScheduleTime::EveningEnd).to_string(),
            },
            "login": state.launch_at_login(),
            "brightnessStatus": state.brightness_status(),
            "colorStatus": state.color_status(),
            "displays": state.displays().iter().map(|d| serde_json::json!({
                "name": d.name,
                "summary": d.capability_summary(),
            })).collect::<Vec<_>>(),
        });
        if cli.json {
            println!("{status}");
        } else {
            print_status(&status);
        }
    }
    // Nudge the DDC worker to flush before exit; gamma stays as applied.
    std::thread::sleep(Duration::from_millis(150));
    0
}

/// Forwards CLI commands to the running instance.
fn forward_command(cli: &Cli) -> i32 {
    if cli.show_settings {
        return match ipc::send(&serde_json::json!({"cmd": "show_settings"})) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
    }
    if cli.quit {
        return match ipc::send(&serde_json::json!({"cmd": "quit"})) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
    }
    let mut acted = false;
    if let Some(percent) = cli.set_brightness {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "set_brightness", "value": percent})) {
            Ok(response) => println!("brightness: {}%", response["brightness"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if cli.brighter || cli.dimmer {
        acted = true;
        let delta = if cli.brighter { 5 } else { -5 };
        match ipc::send(&serde_json::json!({"cmd": "adjust_brightness", "delta": delta})) {
            Ok(response) => println!("brightness: {}%", response["brightness"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    let hotkey_action = if cli.brightness_up {
        Some("increase-brightness")
    } else if cli.brightness_down {
        Some("decrease-brightness")
    } else if cli.temperature_up {
        Some("increase-temperature")
    } else if cli.temperature_down {
        Some("decrease-temperature")
    } else {
        None
    };
    if let Some(action) = hotkey_action {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "hotkey", "action": action})) {
            Ok(response) => {
                println!("brightness: {}%", response["brightness"]);
                println!("temperature: {} K", response["temperature"]);
                println!("automatic: {}", response["auto"]);
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if let Some(kelvin) = cli.set_temp {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "set_temp", "kelvin": kelvin})) {
            Ok(response) => println!("temperature: {} K", response["temperature"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if cli.warmer || cli.cooler {
        acted = true;
        let delta = if cli.warmer {
            -KELVIN_STEP
        } else {
            KELVIN_STEP
        };
        match ipc::send(&serde_json::json!({"cmd": "adjust_temp", "delta": delta})) {
            Ok(response) => println!("temperature: {} K", response["temperature"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if let Some(auto) = &cli.auto {
        acted = true;
        let on = match auto.as_str() {
            "on" => true,
            "off" => false,
            _ => {
                eprintln!("--auto expects 'on' or 'off'.");
                return 2;
            }
        };
        match ipc::send(&serde_json::json!({"cmd": "set_auto", "on": on})) {
            Ok(response) => println!("automatic: {}", response["auto"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if let Some(kelvin) = cli.day_temp {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "set_day_temp", "kelvin": kelvin})) {
            Ok(response) => println!("day temperature: {} K", response["dayTemp"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if let Some(kelvin) = cli.night_temp {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "set_night_temp", "kelvin": kelvin})) {
            Ok(response) => println!("night temperature: {} K", response["nightTemp"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    for (window, pair) in [("morning", &cli.morning), ("evening", &cli.evening)] {
        if let Some(pair) = pair {
            acted = true;
            let (from, to) = match parse_window(window, pair) {
                Ok(times) => times,
                Err(code) => return code,
            };
            match ipc::send(&serde_json::json!({
                "cmd": "set_schedule_window", "window": window,
                "from": from.to_string(), "to": to.to_string(),
            })) {
                Ok(response) => println!("{window}: {} - {}", response["from"], response["to"]),
                Err(error) => {
                    eprintln!("{error}");
                    return 1;
                }
            }
        }
    }
    if let Some(login) = &cli.login {
        acted = true;
        let on = match login.as_str() {
            "on" => true,
            "off" => false,
            _ => {
                eprintln!("--login expects 'on' or 'off'.");
                return 2;
            }
        };
        match ipc::send(&serde_json::json!({"cmd": "set_login", "on": on})) {
            Ok(response) => println!("start at login: {}", response["login"]),
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if cli.status {
        acted = true;
        match ipc::send(&serde_json::json!({"cmd": "status"})) {
            Ok(response) => {
                if cli.json {
                    println!("{response}");
                } else {
                    print_status(&response);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    }
    if acted {
        return 0;
    }
    // Bare run while already running: reveal settings, like opening the app again.
    match ipc::send(&serde_json::json!({"cmd": "show_settings"})) {
        Ok(_) => {
            println!("moonglow is already running; showing settings.");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Owned shutdown resources: worker stops/joins plus the backends whose
/// pre-launch state must be restored (gamma ramps, Night Light, DDC).
struct Shutdown {
    flush_settings: Option<Box<dyn FnOnce() + Send>>,
    sched_stop: Arc<AtomicBool>,
    sched_thread: Option<std::thread::JoinHandle<()>>,
    shortcut_handle: Option<shortcuts::ShortcutHandle>,
    ipc_handle: Option<ipc::IpcHandle>,
    color: Arc<Mutex<dyn ColorTemperatureService>>,
    hardware: Arc<Mutex<ddc::LinuxDdcService>>,
}

impl Shutdown {
    /// Stops producers first, then restores hardware state.
    fn run(self) {
        self.sched_stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.sched_thread {
            let _ = thread.join();
        }
        if let Some(handle) = self.shortcut_handle {
            handle.stop();
        }
        if let Some(handle) = self.ipc_handle {
            handle.stop();
        }
        if let Some(flush) = self.flush_settings {
            flush();
        }
        if let Ok(color) = self.color.lock() {
            color.reset();
        }
        if let Ok(mut ddc) = self.hardware.lock() {
            ddc.dispose();
        }
        eprintln!("moonglow stopped.");
    }
}

type SharedShutdown = Arc<Mutex<Option<Shutdown>>>;

/// Runs shutdown exactly once (first caller wins) and exits the process.
///
/// eframe 0.33 offers no programmatic exit for the root viewport —
/// `ViewportCommand::Close` is silently ignored there — so quit cannot go
/// through the event loop and takes this explicit path instead.
/// Runs the shutdown bundle exactly once; later calls find it taken.
fn run_shutdown_once(shutdown: &SharedShutdown) {
    let owned = shutdown.lock().ok().and_then(|mut guard| guard.take());
    if let Some(shutdown) = owned {
        shutdown.run();
    }
}

/// Grace period for a shutdown already in flight before a late quitter exits
/// anyway. Shutdown normally takes ~10 s (thread joins); the grace only
/// matters if the runner thread died without exiting.
const SHUTDOWN_GRACE_SECS: u64 = 30;

/// Runs the shutdown bundle if still present, then exits the process.
fn quit_now(shutdown: &SharedShutdown) -> ! {
    let owned = shutdown.lock().ok().and_then(|mut guard| guard.take());
    if let Some(shutdown) = owned {
        shutdown.run();
        std::process::exit(0);
    }
    // Another thread took the bundle and is running shutdown: every quit
    // path pairs a direct request with a channel backup, so a second quitter
    // arriving here is the expected case, not a failure. Wait for the
    // runner's exit instead of exiting mid-shutdown, which would skip the
    // hardware restore (lost Night Light schedule, stuck gamma) and the
    // stopped marker.
    for _ in 0..SHUTDOWN_GRACE_SECS {
        std::thread::sleep(Duration::from_secs(1));
    }
    std::process::exit(0);
}

/// Shutdown bundle for out-of-loop quit requests (IPC, Ctrl-C).
static SHUTDOWN: std::sync::OnceLock<SharedShutdown> = std::sync::OnceLock::new();

/// Quits from any thread without involving the event loop: shutdown runs on a
/// dedicated thread (the caller is never joined into itself) and then exits.
/// A minimized window starves `update()` of redraws, so quit cannot wait for it.
pub fn request_quit() {
    if let Some(shutdown) = SHUTDOWN.get() {
        let shutdown = shutdown.clone();
        let _ = std::thread::Builder::new()
            .name("moonglow-quit".to_string())
            .spawn(move || quit_now(&shutdown));
    }
}

fn main() {
    let cli = Cli::parse();

    // Serialize startup before any backend can capture or change global display state.
    let _instance_lock = match ipc::InstanceLock::try_acquire() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            while std::time::Instant::now() < deadline {
                if ipc::ping() {
                    std::process::exit(forward_command(&cli));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            eprintln!("another Moonglow instance is starting or busy; try again shortly");
            std::process::exit(1);
        }
        Err(error) => {
            eprintln!("cannot acquire Moonglow instance lock: {error}");
            std::process::exit(1);
        }
    };
    if ipc::ping() {
        std::process::exit(forward_command(&cli));
    }

    let one_shot = cli.set_brightness.is_some()
        || cli.set_temp.is_some()
        || cli.auto.is_some()
        || cli.day_temp.is_some()
        || cli.night_temp.is_some()
        || cli.morning.is_some()
        || cli.evening.is_some()
        || cli.login.is_some()
        || cli.status
        || cli.quit
        || cli.brighter
        || cli.dimmer
        || cli.warmer
        || cli.cooler
        || cli.brightness_up
        || cli.brightness_down
        || cli.temperature_up
        || cli.temperature_down;
    if one_shot && !cli.show_settings {
        std::process::exit(direct_command(&cli));
    }

    // Long-running instance only: one-shots keep the terminal.
    if let Some(path) = logging::init() {
        println!("log: {}", path.display());
    }

    let composed = compose_backends();
    let Composed {
        display,
        hardware,
        color,
        brightness,
    } = composed;

    let store = store::SettingsStore::default_path();
    let login = autostart::AutostartService::default_path();
    let state: SharedState<store::SettingsStore> = Arc::new(Mutex::new(StateManager::new(
        store,
        display,
        Arc::new(Mutex::new(brightness)),
        color.clone(),
        Box::new(login),
    )));
    {
        let mut state = state.lock().expect("state lock");
        state.apply_login_item_state(is_first_launch_after_install());
        state.apply_initial_state();
    }

    let (cmd_tx, cmd_rx) = mpsc::channel::<AppCommand>();
    {
        let tx = cmd_tx.clone();
        if let Ok(mut state) = state.lock() {
            state.on_show_settings(move || {
                let _ = tx.send(AppCommand::ShowWindow);
                settings_ui::wake_ui();
            });
        }
        let tx = cmd_tx.clone();
        if let Ok(mut state) = state.lock() {
            state.on_quit(move || {
                let _ = tx.send(AppCommand::Quit);
                settings_ui::wake_ui();
            });
        }
    }

    // Schedule thread: re-evaluates every 10 s, immediately on start.
    let sched_stop = Arc::new(AtomicBool::new(false));
    let sched_state = state.clone();
    let sched_stop_flag = sched_stop.clone();
    let sched_thread = std::thread::Builder::new()
        .name("moonglow-schedule".to_string())
        .spawn(move || {
            if let Ok(mut s) = sched_state.lock() {
                // Displays first: a changed set re-probes brightness
                // hardware before the schedule applies to fresh state.
                s.refresh_displays();
                s.tick_schedule();
            }
            while !sched_stop_flag.load(Ordering::Relaxed) {
                // Sleep in short slices so shutdown never waits out the tick.
                for _ in 0..TICK_INTERVAL_SECS {
                    if sched_stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                if sched_stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(mut s) = sched_state.lock() {
                    s.refresh_displays();
                    s.tick_schedule();
                }
            }
        })
        .ok();

    // IPC always runs on the primary so later invocations forward.
    let ipc_handle = Some(
        ipc::spawn_ipc(state.clone(), cmd_tx.clone()).unwrap_or_else(|error| {
            eprintln!("{error}");
            if let Ok(color) = color.lock() {
                color.reset();
            }
            std::process::exit(1);
        }),
    );

    // Global shortcuts: registered by the desktop or grabbed on X11, then
    // re-registered whenever the bindings change.
    let session_kind = session::detect_session();
    let (shortcut_handle, shortcut_status) = shortcuts::spawn(state.clone(), session_kind);
    let shortcut_handle = Some(shortcut_handle);

    // Ctrl-C quits gracefully through the same path as the window button.
    {
        let tx = cmd_tx.clone();
        let _ = ctrlc::set_handler(move || {
            // Direct first (never waits for the event loop), channel as backup.
            request_quit();
            let _ = tx.send(AppCommand::Quit);
            settings_ui::wake_ui();
        });
    }

    // Quit owns this bundle: out-of-loop requests take and run it, so the
    // event loop is never on the shutdown path (see `quit_now`).
    let shutdown: SharedShutdown = Arc::new(Mutex::new(Some(Shutdown {
        flush_settings: Some(Box::new({
            let state = state.clone();
            move || {
                if let Ok(state) = state.lock() {
                    state.flush_settings();
                }
            }
        })),
        sched_stop,
        sched_thread,
        shortcut_handle,
        ipc_handle,
        color,
        hardware,
    })));
    let _ = SHUTDOWN.set(shutdown.clone());

    if cli.headless {
        eprintln!("running headless (schedule + shortcuts); quit with Ctrl-C or --quit");
        loop {
            match cmd_rx.recv() {
                Ok(AppCommand::Quit) => break,
                Ok(AppCommand::ShowWindow) => {
                    eprintln!("settings window unavailable in --headless mode");
                }
                Err(_) => break,
            }
        }
    } else {
        // winit cannot hide windows on Wayland (`set_visible` is a documented
        // no-op there), which breaks the hide/show lifecycle into stuck empty
        // windows. Run the settings window through XWayland, where hide/unmap
        // works; session and backends were already detected from the real
        // environment above, so colour, shortcuts, and IPC select the native
        // Wayland paths regardless.
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            std::env::remove_var("WAYLAND_DISPLAY");
        }
        // The settings window owns the main thread (GUI toolkits require it)
        // and polls the same command channel every frame. A normal launch is
        // visible; `--hidden` starts in the background for autostart.
        let start_visible = cli.show_settings || !cli.hidden;
        if let Err(e) = settings_ui::run_settings_window(
            state.clone(),
            cmd_rx,
            shortcut_status,
            start_visible,
            shutdown.clone(),
        ) {
            eprintln!("{e}");
            eprintln!("hint: no graphical display? Run with --headless instead.");
        }
    }

    // Shutdown fallback if the event loop ever returns on its own (or the
    // headless loop breaks).
    run_shutdown_once(&shutdown);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn settings_flags_parse() {
        let cli = Cli::try_parse_from([
            "moonglow",
            "--day-temp",
            "6000",
            "--night-temp",
            "2700",
            "--morning",
            "06:30",
            "09:00",
            "--evening",
            "18:00",
            "21:30",
            "--login",
            "on",
        ])
        .unwrap();
        assert_eq!(cli.day_temp, Some(6000));
        assert_eq!(cli.night_temp, Some(2700));
        assert_eq!(
            cli.morning,
            Some(vec!["06:30".to_string(), "09:00".to_string()])
        );
        assert_eq!(
            cli.evening,
            Some(vec!["18:00".to_string(), "21:30".to_string()])
        );
        assert_eq!(cli.login, Some("on".to_string()));
    }

    #[test]
    fn cli_catalog_matches_clap_flags() {
        use clap::CommandFactory;
        let mut actual: Vec<String> = Cli::command()
            .get_arguments()
            .filter_map(|a| a.get_long().map(str::to_string))
            .collect();
        actual.sort();
        let mut catalog: Vec<String> = crate::settings_ui::CLI_COMMANDS
            .iter()
            .map(|c| c.name.to_string())
            .collect();
        catalog.sort();
        assert_eq!(catalog, actual);
        for command in crate::settings_ui::CLI_COMMANDS {
            assert!(
                command.usage.contains(command.name),
                "usage shows its flag: {}",
                command.usage
            );
        }
    }

    #[test]
    fn window_pairs_validate_before_applying() {
        let pair = vec!["06:30".to_string(), "09:00".to_string()];
        let (from, to) = parse_window("--morning", &pair).unwrap();
        assert_eq!((from.hour, from.minute), (6, 30));
        assert_eq!((to.hour, to.minute), (9, 0));
        // One bad time rejects the whole pair.
        let bad = vec!["06:30".to_string(), "25:99".to_string()];
        assert_eq!(parse_window("--morning", &bad), Err(2));
        let short = vec!["06:30".to_string()];
        assert_eq!(parse_window("--morning", &short), Err(2));
    }

    struct CountingColor {
        resets: Arc<Mutex<usize>>,
    }

    impl ColorTemperatureService for CountingColor {
        fn backend_name(&self) -> String {
            "test".to_string()
        }
        fn is_supported(&self) -> bool {
            true
        }
        fn apply(&self, _kelvin: i32) -> bool {
            true
        }
        fn reset(&self) {
            *self.resets.lock().unwrap() += 1;
        }
    }

    /// The shutdown bundle runs exactly once no matter how many quit paths
    /// fire: the take() arbitrates, so a late backup finds it taken instead
    /// of running (or skipping) the hardware restore a second time.
    #[test]
    fn shutdown_bundle_runs_exactly_once() {
        let resets = Arc::new(Mutex::new(0usize));
        let color: Arc<Mutex<dyn ColorTemperatureService>> = Arc::new(Mutex::new(CountingColor {
            resets: resets.clone(),
        }));
        let shutdown: SharedShutdown = Arc::new(Mutex::new(Some(Shutdown {
            flush_settings: None,
            sched_stop: Arc::new(AtomicBool::new(false)),
            sched_thread: None,
            shortcut_handle: None,
            ipc_handle: None,
            color,
            hardware: Arc::new(Mutex::new(ddc::LinuxDdcService::probe())),
        })));
        run_shutdown_once(&shutdown);
        run_shutdown_once(&shutdown);
        assert_eq!(*resets.lock().unwrap(), 1);
        assert!(shutdown.lock().unwrap().is_none());
    }
}
