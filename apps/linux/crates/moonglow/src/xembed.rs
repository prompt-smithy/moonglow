//! Legacy X11 system-tray (XEmbed) fallback.
//!
//! Used only on X11 sessions where no StatusNotifierWatcher registers us
//! (minimal panels like tint2/stalonetray). Wires a 24px dock window into
//! `_NET_SYSTEM_TRAY_S0`, paints a small moon glyph, and opens the settings
//! window on click. No menu: fine control lives in the settings window and CLI.
//!
//! This protocol is X11-only and long superseded by StatusNotifier; it exists
//! so the app has *some* presence on every system.

use crate::tray::TrayCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    self, ChangeGCAux, ClientMessageData, ClientMessageEvent, ConnectionExt as _, CreateGCAux,
    CreateWindowAux, EventMask, PropMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

/// Opcode: dock the client window into the tray.
const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;
/// XEmbed protocol version we speak.
const XEMBED_VERSION: u32 = 0;
/// XEmbed mapped flag.
const XEMBED_MAPPED: u32 = 1;

/// Builds the `data.l` payload of a dock request: timestamp, opcode, window.
fn dock_request_data(timestamp: u32, window: Window) -> [u32; 5] {
    [timestamp, SYSTEM_TRAY_REQUEST_DOCK, window, 0, 0]
}

/// Handle to the running fallback thread.
pub struct XEmbedHandle {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl XEmbedHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Docks into the X11 tray on a background thread. Fails cleanly without an
/// X server or tray owner — the app keeps running trayless.
pub fn spawn_xembed(cmd_tx: mpsc::Sender<TrayCommand>) -> Result<XEmbedHandle, String> {
    // Probe synchronously so failure is known before the thread exists.
    let (conn, screen) = x11rb::connect(None).map_err(|e| format!("xembed: no X server ({e})"))?;
    let screen_info = conn.setup().roots[screen].clone();
    let tray_atom = intern(&conn, b"_NET_SYSTEM_TRAY_S0")?;
    let owner = conn
        .get_selection_owner(tray_atom)
        .map_err(|e| format!("xembed: selection query failed ({e})"))?
        .reply()
        .map_err(|e| format!("xembed: selection reply failed ({e})"))?
        .owner;
    if owner == x11rb::NONE {
        return Err("xembed: no tray owner".to_string());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("moonglow-xembed".to_string())
        .spawn(move || serve(conn, screen, screen_info, owner, cmd_tx, stop_flag))
        .map_err(|e| format!("xembed thread failed: {e}"))?;
    Ok(XEmbedHandle {
        stop,
        thread: Some(thread),
    })
}

fn intern(conn: &RustConnection, name: &[u8]) -> Result<xproto::Atom, String> {
    conn.intern_atom(false, name)
        .map_err(|e| format!("xembed: intern failed ({e})"))?
        .reply()
        .map(|r| r.atom)
        .map_err(|e| format!("xembed: intern reply failed ({e})"))
}

#[allow(clippy::too_many_arguments)]
fn serve(
    conn: RustConnection,
    screen: usize,
    screen_info: xproto::Screen,
    tray_owner: Window,
    cmd_tx: mpsc::Sender<TrayCommand>,
    stop: Arc<AtomicBool>,
) {
    let win = match dock_window(&conn, &screen_info, tray_owner) {
        Ok(win) => win,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };
    let gc = match conn.generate_id() {
        Ok(gc) => gc,
        Err(_) => return,
    };
    if conn
        .create_gc(gc, win, &CreateGCAux::new().graphics_exposures(0))
        .is_err()
    {
        return;
    }
    let _ = conn.map_window(win);
    let _ = conn.flush();

    let mut size = (24u16, 24u16);
    paint(&conn, &screen_info, gc, win, size);
    while !stop.load(Ordering::Relaxed) {
        match conn.poll_for_event() {
            Ok(Some(event)) => match event {
                x11rb::protocol::Event::Expose(_) => {
                    paint(&conn, &screen_info, gc, win, size);
                }
                x11rb::protocol::Event::ConfigureNotify(n) => {
                    size = (n.width, n.height);
                    paint(&conn, &screen_info, gc, win, size);
                }
                x11rb::protocol::Event::ButtonPress(_) => {
                    let _ = cmd_tx.send(TrayCommand::ShowSettings);
                    crate::settings_ui::wake_ui();
                }
                _ => {}
            },
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break,
        }
    }
    let _ = conn.destroy_window(win);
    let _ = conn.flush();
}

fn dock_window(
    conn: &RustConnection,
    screen: &xproto::Screen,
    tray_owner: Window,
) -> Result<Window, String> {
    let win = conn
        .generate_id()
        .map_err(|e| format!("xembed: no window id ({e})"))?;
    let aux = CreateWindowAux::new()
        .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::STRUCTURE_NOTIFY)
        .background_pixel(screen.black_pixel);
    conn.create_window(
        screen.root_depth,
        win,
        screen.root,
        0,
        0,
        24,
        24,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &aux,
    )
    .map_err(|e| format!("xembed: window failed ({e})"))?;

    // XEMBED_INFO = [version, mapped].
    let xembed_info = intern(conn, b"_XEMBED_INFO")?;
    let info: Vec<u8> = [XEMBED_VERSION, XEMBED_MAPPED]
        .iter()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    conn.change_property(
        PropMode::REPLACE,
        win,
        xembed_info,
        xembed_info,
        32,
        2,
        &info,
    )
    .map_err(|e| format!("xembed: property failed ({e})"))?;

    // Ask the tray to embed us.
    let opcode = intern(conn, b"_NET_SYSTEM_TRAY_OPCODE")?;
    let event = ClientMessageEvent::new(
        32,
        win,
        opcode,
        ClientMessageData::from(dock_request_data(0, win)),
    );
    conn.send_event(false, tray_owner, EventMask::default(), event)
        .map_err(|e| format!("xembed: dock failed ({e})"))?;
    conn.flush()
        .map_err(|e| format!("xembed: flush failed ({e})"))?;
    Ok(win)
}

/// Paints a crescent moon: dark disc, amber disc, dark bite.
fn paint(
    conn: &RustConnection,
    screen: &xproto::Screen,
    gc: u32,
    win: Window,
    (width, height): (u16, u16),
) {
    let amber = alloc_color(conn, screen, 0xE896, 0x6432, 0x1E1E);
    let dark = screen.black_pixel;
    let w = width as i16;
    let h = height as i16;
    let _ = set_fg(conn, gc, dark);
    let _ = conn.poly_fill_rectangle(
        win,
        gc,
        &[xproto::Rectangle {
            x: 0,
            y: 0,
            width,
            height,
        }],
    );
    let d = w.min(h) as u16;
    let _ = set_fg(conn, gc, amber);
    let _ = conn.poly_fill_arc(
        win,
        gc,
        &[xproto::Arc {
            x: (w - d as i16) / 2,
            y: (h - d as i16) / 2,
            width: d.saturating_sub(4).max(4),
            height: d.saturating_sub(4).max(4),
            angle1: 0,
            angle2: 360 * 64,
        }],
    );
    // Bite out the crescent.
    let _ = set_fg(conn, gc, dark);
    let bite = d / 2;
    let _ = conn.poly_fill_arc(
        win,
        gc,
        &[xproto::Arc {
            x: (w - d as i16) / 2 + d as i16 / 4,
            y: (h - d as i16) / 2 - 1,
            width: bite.max(4),
            height: bite.max(4),
            angle1: 0,
            angle2: 360 * 64,
        }],
    );
    let _ = conn.flush();
}

fn set_fg(
    conn: &RustConnection,
    gc: u32,
    pixel: u32,
) -> Result<(), x11rb::errors::ConnectionError> {
    conn.change_gc(gc, &ChangeGCAux::new().foreground(pixel))?;
    Ok(())
}

fn alloc_color(conn: &RustConnection, screen: &xproto::Screen, r: u16, g: u16, b: u16) -> u32 {
    conn.alloc_color(screen.default_colormap, r, g, b)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|reply| reply.pixel)
        .unwrap_or(screen.white_pixel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_request_encodes_opcode_and_window() {
        assert_eq!(dock_request_data(1234, 0x42), [1234, 0, 0x42, 0, 0]);
    }

    #[test]
    fn spawn_fails_cleanly_without_x() {
        // Must fail gracefully, never claim a tray it cannot dock into. If an
        // X server IS reachable the outcome varies, but it must not panic.
        // (A stray success here would dock a real icon; restrict to probe.)
        if std::env::var("DISPLAY").is_err() {
            assert!(spawn_xembed(mpsc::channel().0).is_err());
        }
    }
}
