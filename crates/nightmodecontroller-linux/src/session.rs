//! Session and compositor detection.
//!
//! Gamma control on Linux is per-compositor: X11 sessions use RandR gamma,
//! wlroots compositors use `wlr-gamma-control`, GNOME and KDE expose their own
//! night-light APIs. Detection reads the standard session environment and each
//! backend still probes for real before reporting support — a capability is
//! never reported unless the backend can actually apply it.

/// The running graphical session, as far as environment variables reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// X11 (`DISPLAY` set, no `WAYLAND_DISPLAY`).
    X11,
    /// Wayland GNOME (Mutter): night light over D-Bus + gsettings.
    GnomeWayland,
    /// Wayland KDE (KWin): night light preview over D-Bus.
    KdeWayland,
    /// Wayland on a wlroots-based compositor: `wlr-gamma-control`.
    WlrWayland,
    /// Wayland on an unrecognized compositor: probe `wlr-gamma-control` anyway.
    UnknownWayland,
    /// XWayland (both set): RandR gamma usually reaches through, but probing decides.
    XWayland,
    /// No graphical session detectable.
    Headless,
}

/// Reads the session kind from environment variables.
pub fn detect_session() -> SessionKind {
    detect_from_env(
        &std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        &std::env::var("WAYLAND_DISPLAY").unwrap_or_default(),
        &std::env::var("DISPLAY").unwrap_or_default(),
        &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
    )
}

fn detect_from_env(
    session_type: &str,
    wayland_display: &str,
    display: &str,
    desktop: &str,
) -> SessionKind {
    let session_type = session_type.to_lowercase();
    let desktop = desktop.to_lowercase();
    let has_wayland = !wayland_display.is_empty() || session_type == "wayland";
    let has_x11 = !display.is_empty();

    if has_wayland && has_x11 && session_type != "wayland" {
        // Ambiguous: both set without an explicit session type. When the type
        // says X11 it is X11; otherwise treat XWayland-capable as XWayland.
        if session_type == "x11" {
            return SessionKind::X11;
        }
    }

    if has_wayland {
        if desktop.contains("gnome") {
            return SessionKind::GnomeWayland;
        }
        if desktop.contains("kde") || desktop.contains("plasma") {
            return SessionKind::KdeWayland;
        }
        if has_x11 && session_type != "wayland" {
            return SessionKind::XWayland;
        }
        if desktop.contains("sway")
            || desktop.contains("hyprland")
            || desktop.contains("wayfire")
            || desktop.contains("river")
            || desktop.contains("labwc")
        {
            return SessionKind::WlrWayland;
        }
        return SessionKind::UnknownWayland;
    }
    if has_x11 {
        return SessionKind::X11;
    }
    SessionKind::Headless
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_sessions() {
        assert_eq!(detect_from_env("x11", "", ":0", "GNOME"), SessionKind::X11);
        assert_eq!(
            detect_from_env("wayland", "wayland-0", ":0", "GNOME"),
            SessionKind::GnomeWayland
        );
        assert_eq!(
            detect_from_env("wayland", "wayland-0", "", "KDE"),
            SessionKind::KdeWayland
        );
        assert_eq!(
            detect_from_env("wayland", "wayland-1", "", "Hyprland"),
            SessionKind::WlrWayland
        );
        assert_eq!(
            detect_from_env("wayland", "wayland-1", "", "niri"),
            SessionKind::UnknownWayland
        );
        assert_eq!(detect_from_env("", "", "", ""), SessionKind::Headless);
    }
}
