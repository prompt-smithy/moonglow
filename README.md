# Night Mode Controller for Linux

Rust port of the macOS menu-bar app: a tray app for external-display hardware
brightness (DDC/CI) and scheduled colour temperature. One static-ish binary,
zero system GUI dependencies — it links only libc — so the same build runs on
Ubuntu, Debian, and Arch.

## Layout

- `crates/nmc-core/` — platform logic ported from Swift `AmberKit`: settings,
  schedule math, Kelvin conversion, DDC/CI packets, display model, shared state
  manager. No UI or OS dependency.
- `crates/nightmodecontroller-linux/` — Linux app: session backends, tray,
  settings window, CLI, IPC.
- `packaging/` — `.desktop` launcher, udev rule for DDC/CI, `.deb` build script,
  Arch `PKGBUILD` template.

## Backends

Colour temperature is per-session; every backend probes for real before the app
claims support:

- X11 / XWayland — RandR gamma tables (temperature + software brightness).
- GNOME Wayland — SettingsDaemon Night Light over D-Bus (`Temperature`
  property) with `gsettings` fallback. Temperature only.
- KDE Wayland — KWin `NightLight.preview()` over D-Bus, held and refreshed.
  Temperature only.
- wlroots (Sway, Hyprland, Wayfire, ...) — `wlr-gamma-control` (temperature +
  software brightness).
- Brightness everywhere — DDC/CI over `/dev/i2c-*` on every panel that answers
  the luminance probe; software gamma dimming where the session backend allows.

The app restores the pre-launch state on quit (original gamma ramps, Night
Light settings, KWin preview stopped).

## Requirements

- Ubuntu 24.04+ / Debian 12+ / Arch, x86_64 or aarch64.
- Rust stable and a C toolchain (`build-essential` / `base-devel`) to compile.
- For DDC/CI: the `i2c-dev` module, membership in the `i2c` group, and the
  shipped udev rule (installed by both packages).
- For the tray icon: a StatusNotifier host — KDE Plasma built in, Ubuntu GNOME
  via the preinstalled AppIndicator extension, wlroots bars via their SNI
  module. Vanilla GNOME needs either that extension or the bundled Shell
  indicator below; minimal X11 panels get a legacy XEmbed fallback icon. With
  no host at all the app still runs — use the settings window and CLI.

## Install

Debian / Ubuntu / Mint / Pop!_OS (from the GitHub release `.deb`):

```bash
sudo apt install ./nightmodecontroller-linux_1.0.0_amd64.deb
```

Or build it yourself (version comes from `Cargo.toml`):

```bash
bash packaging/build-deb.sh        # → dist/nightmodecontroller-linux_*_amd64.deb
sudo apt install ./dist/nightmodecontroller-linux_*_amd64.deb
```

Arch (AUR):

```bash
yay -S nightmodecontroller-linux
# or: cd packaging/arch && makepkg -si
```

Fedora / openSUSE (COPR — see `packaging/rpm/COPR.md` for publishing):

```bash
sudo dnf copr enable <you>/nightmodecontroller-linux
sudo dnf install nightmodecontroller-linux
```

Or run from source:

```bash
cargo run --release -p nightmodecontroller-linux
```

DDC/CI permission (all distros — the installers print this too):

```bash
sudo modprobe i2c-dev
sudo usermod -aG i2c "$USER"
# log out and back in, then check:
nightmodecontroller-linux --status
```

If brightness reports "permission denied" naming your I2C buses, the step
above is what is missing: `/dev/i2c-*` ships root-only, and your monitor
working on Windows only proves the panel and cable are fine — Linux still
needs the group + udev rule before any program may open the bus. (All three
packages install the udev rule and create the `i2c` group; joining the group
is always manual.)

## Use

Launch `nightmodecontroller-linux` (or the app launcher entry). It starts hidden
with a tray icon: brightness/temperature steppers, the automatic toggle,
settings, and quit live there. Scrolling on the icon adjusts in place with a
small value bubble for feedback. The settings window mirrors macOS — schedule
pickers, the day/night range as one two-handle bar, startup toggle, displays —
and hides instead of closing.

A second invocation forwards to the running instance:

```bash
nightmodecontroller-linux --show-settings
nightmodecontroller-linux --set-brightness 70
nightmodecontroller-linux --set-temp 4000
nightmodecontroller-linux --auto off
nightmodecontroller-linux --day-temp 6500 --night-temp 1300
nightmodecontroller-linux --morning 06:00 09:00 --evening 18:00 21:00
nightmodecontroller-linux --login on
nightmodecontroller-linux --status
nightmodecontroller-linux --quit
```

With no daemon running, the one-shot flags drive the backends directly (handy
headless or in scripts). `--headless` runs schedule + tray with no window.
Relative steppers suit keyboard shortcuts (Settings › Keyboard › Custom
Shortcuts):

```bash
nightmodecontroller-linux --brighter   # or --dimmer (5%)
nightmodecontroller-linux --warmer     # or --cooler (250 K, manual mode)
nightmodecontroller-linux --status --json   # machine-readable, for scripts
```

Diagnostics go to `~/.local/state/nightmodecontroller-linux/nightmodecontroller-linux.log`
(`$XDG_STATE_HOME` when set), rotated past 2 MB to `nightmodecontroller-linux.log.1`.
One-shot CLI output stays on the terminal; only the long-running instance logs.

## Tray on every desktop

Presence is layered so something always shows:

1. StatusNotifierItem (built in): KDE Plasma, Ubuntu GNOME, Cinnamon, MATE,
   XFCE (StatusNotifier plugin), wlroots bars (e.g. waybar `tray` module).
2. GNOME Shell indicator (`packaging/gnome-shell-extension/`): for vanilla
   GNOME without the AppIndicator extension. Needs the binary on PATH:
   `bash packaging/gnome-shell-extension/install.sh`. If the Shell does not
   list it immediately, log out and back in.
3. XEmbed fallback (built in, X11 only): if no StatusNotifierWatcher registers
   within 8 seconds on an X11 session, the app docks a legacy icon into
   `_NET_SYSTEM_TRAY_S0` instead. Click opens settings.

Settings live at `~/.config/nightmodecontroller-linux/settings.json` (same schema as macOS).
Start-at-login is a `~/.config/autostart` entry managed from the settings
window; a registration the desktop refuses is never recorded.

## Divergences from macOS

- Settings window is egui, not GTK/Qt: a deliberate trade for a single binary
  with no system GUI dependencies and identical rendering on every distro.
- Appearance always follows the desktop (no manual Light/Dark switch).
- Night-light backends clamp to their own floors (1000 K); X11/wlroots cover
  the full 900–6600 K range.
- DDC/CI drives every answering panel, not just the primary external display.
- CLI flags and the Unix-socket IPC have no macOS equivalent.

## Develop

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

## Verification checklist

Needs hardware and real sessions; not automatable.

- Gamma apply + restore on quit on X11 and one wlroots compositor.
- DDC brightness writes over HDMI/DisplayPort/USB-C (check `i2c` group path).
- GNOME Night Light drive + original settings restored on quit.
- KWin preview held across schedule ticks + `stopPreview` on quit.
- Tray menu on Plasma and on GNOME + AppIndicator.
- All five schedule states, including both transitions.
- Settings round trip through `~/.config/nightmodecontroller-linux/settings.json`.
- Second invocation forwards; `--quit` stops the daemon; socket removed.
