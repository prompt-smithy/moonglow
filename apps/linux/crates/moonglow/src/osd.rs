//! Transient value feedback for tray adjustments.
//!
//! dbusmenu popups close on every click and scroll wheels give no feedback,
//! so each tray adjustment (scroll or menu stepper) shows a desktop
//! notification bubble with the new value — the volume/brightness OSD
//! pattern. Bubbles replace the previous one instead of stacking
//! (`replaces_id` plus the synchronous hint honoured across GNOME, KDE,
//! and Xfce). Bubbles are text-only (title plus one value line): no `value`
//! hint, so servers that render a progress bar row omit it and the bubble
//! stays as small as the server's fixed chrome allows.
//!
//! A dedicated worker thread owns the notification connection: tray handlers
//! run inside the tray runtime, where opening a second connection would nest
//! runtimes, and the settings thread may be starved while hidden. Bursts
//! drain to the freshest value before notifying, so a fast scroll shows one
//! bubble with the latest value instead of a queue of stale ones.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use zbus::zvariant::{OwnedValue, Value};

/// One feedback bubble: a title plus a single value line.
pub struct OsdRequest {
    pub summary: String,
    pub body: String,
    pub icon: &'static str,
}

/// Bubble for a brightness level 1...100.
pub fn brightness_request(percent: i32) -> OsdRequest {
    let percent = percent.clamp(0, 100);
    OsdRequest {
        summary: "Brightness".to_string(),
        body: format!("{percent}%"),
        icon: super::tray::BRIGHTNESS_ICON_NAME,
    }
}

/// Bubble for a colour temperature: the text is the exact kelvin value.
pub fn temperature_request(kelvin: i32) -> OsdRequest {
    OsdRequest {
        summary: "Colour temperature".to_string(),
        body: format!("{kelvin} K"),
        icon: super::tray::COLOR_ICON_NAME,
    }
}

/// Hints for a replacing, transient, text-only OSD bubble.
///
/// No `value` hint (it adds a progress bar row on servers that render it)
/// and no urgency hint: urgency low suppresses the banner on GNOME (queued
/// silently instead), so the server default (normal) is what we want.
fn bubble_hints() -> HashMap<String, OwnedValue> {
    let mut hints = HashMap::new();
    // One bubble at a time: servers replace the previous bubble carrying
    // the same synchronous key instead of stacking a new one.
    if let Ok(value) = OwnedValue::try_from(Value::new("moonglow-osd")) {
        hints.insert("x-canonical-private-synchronous".to_string(), value);
    }
    if let Ok(value) = OwnedValue::try_from(Value::Bool(true)) {
        hints.insert("transient".to_string(), value);
    }
    hints
}

/// Spawns the worker thread and returns its request channel. Without a
/// session bus the thread exits and sends fail silently — feedback is
/// best-effort, never load-bearing.
pub fn spawn() -> mpsc::Sender<OsdRequest> {
    let (tx, rx) = mpsc::channel::<OsdRequest>();
    let _ = std::thread::Builder::new()
        .name("moonglow-osd".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            runtime.block_on(serve(rx));
        });
    tx
}

async fn serve(rx: mpsc::Receiver<OsdRequest>) {
    let conn = match zbus::Connection::session().await {
        Ok(conn) => conn,
        Err(_) => return,
    };
    let proxy = match zbus::Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(_) => return,
    };
    let warned = AtomicBool::new(false);
    let mut replaces_id: u32 = 0;
    while let Ok(first) = rx.recv() {
        // Drain bursts: only the freshest value notifies.
        let mut latest = first;
        while let Ok(next) = rx.try_recv() {
            latest = next;
        }
        match notify(&proxy, &latest, replaces_id).await {
            Ok(id) => replaces_id = id,
            Err(_) => {
                // One warning, then silence: a missing notification daemon
                // must not spam the log on every scroll notch.
                if !warned.swap(true, Ordering::Relaxed) {
                    eprintln!("osd: notification daemon unreachable; value bubbles off");
                }
            }
        }
    }
}

async fn notify(
    proxy: &zbus::Proxy<'_>,
    request: &OsdRequest,
    replaces_id: u32,
) -> Result<u32, zbus::Error> {
    let reply = proxy
        .call_method(
            "Notify",
            &(
                "Moonglow",
                replaces_id,
                request.icon,
                request.summary.clone(),
                request.body.clone(),
                Vec::<String>::new(),
                bubble_hints(),
                // Long enough to read the value; bursts replace anyway.
                2500i32,
            ),
        )
        .await?;
    reply.body().deserialize::<u32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_bubble_carries_percent() {
        let request = brightness_request(48);
        assert_eq!(request.summary, "Brightness");
        assert_eq!(request.body, "48%");
        assert_eq!(brightness_request(150).body, "100%");
        assert_eq!(brightness_request(-5).body, "0%");
    }

    #[test]
    fn temperature_bubble_shows_kelvin() {
        let request = temperature_request(6000);
        assert_eq!(request.summary, "Colour temperature");
        assert_eq!(request.body, "6000 K");
    }

    #[test]
    fn bubble_hints_replace_without_bar() {
        let hints = bubble_hints();
        assert!(hints.contains_key("x-canonical-private-synchronous"));
        // Text-only bubble: no `value` hint, so servers that render a
        // progress bar row omit it and the bubble stays one line tall.
        assert!(!hints.contains_key("value"));
        assert!(hints.contains_key("transient"));
        // Low urgency suppresses the banner on GNOME: never hint it.
        assert!(!hints.contains_key("urgency"));
    }

    #[test]
    fn spawn_without_bus_stays_silent() {
        // No assertions on delivery (no daemon in CI): sending must neither
        // panic nor block when nobody receives.
        let tx = spawn();
        let _ = tx.send(brightness_request(50));
    }
}
