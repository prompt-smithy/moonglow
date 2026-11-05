# Moonglow for Linux

Desktop application for external-display brightness (DDC/CI), scheduled colour
temperature, and global keyboard shortcuts. The settings window is a native
Adwaita (GTK4 + libadwaita) preferences window, so the same build runs on
Ubuntu, Debian, and Arch wherever those system libraries are installed (the
packages declare them).

## Layout

- `crates/moonglow-core/` — platform logic ported from Swift `MoonglowKit`: settings,
  schedule math, Kelvin conversion, shortcut model, DDC/CI packets, display model,
  shared state manager. No UI or OS dependency.
- `crates/moonglow/` — Linux app: session backends, settings window, global
  shortcuts, CLI, IPC.
- `packaging/` — `.desktop` launcher, udev rule for DDC/CI, `.deb` build script,
  Arch `PKGBUILD` template. Icons come from the shared
  [`resources/linux/hicolor`](../../resources/linux/hicolor) set at the repository root.

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
- GTK4 + libadwaita development files (`libgtk-4-dev libadwaita-1-dev` on
  Debian/Ubuntu, `gtk4 libadwaita` on Arch, `gtk4-devel libadwaita-devel` on
  Fedora/openSUSE). The UI only uses APIs available in GTK 4.8 / libadwaita
  1.2, so Debian 12 stays supported.
- For DDC/CI: the `i2c-dev` module, membership in the `i2c` group, and the
  shipped udev rule (installed by both packages).
- No tray host or GNOME extension is needed.

## Install

Debian / Ubuntu / Mint / Pop!_OS (from the GitHub release `.deb`):

```bash
sudo apt install ./moonglow_1.0.1_amd64.deb
```

Or build it yourself (version comes from `Cargo.toml`):

```bash
bash packaging/build-deb.sh        # → ../../release/1.0.1/linux/moonglow_*_amd64.deb
sudo apt install ./../../release/1.0.1/linux/moonglow_*_amd64.deb
```

Arch (AUR):

```bash
yay -S moonglow
# or: cd packaging/arch && makepkg -si
```

Fedora / openSUSE (COPR — see `packaging/rpm/COPR.md` for publishing):

```bash
sudo dnf copr enable <you>/moonglow
sudo dnf install moonglow
```

Or run from source:

```bash
cargo run --release -p moonglow
```

DDC/CI permission (all distros — the installers print this too):

```bash
sudo modprobe i2c-dev
sudo usermod -aG i2c "$USER"
# log out and back in, then check:
moonglow --status
```

If brightness reports "permission denied" naming your I2C buses, the step
above is what is missing: `/dev/i2c-*` ships root-only, and your monitor
working on Windows only proves the panel and cable are fine — Linux still
needs the group + udev rule before any program may open the bus. (All three
packages install the udev rule and create the `i2c` group; joining the group
is always manual.)

## Use

Launch `moonglow` (or the app launcher entry). The settings window opens with:

- **Brightness** — a slider for the hardware brightness of every answering
  DDC/CI panel.
- **Colour temperature** — a warm-to-cool slider, the automatic schedule
  switch, and the day/night range as one two-handle bar. The slider is
  disabled while Automatic is on; the schedule is driving the display.
- **Shortcuts** — four global shortcuts: brightness up/down and temperature
  up/down. Click a shortcut, press the new combination, and it is registered
  immediately. `Clear` removes one.
- **Schedule** — morning and evening transition windows.
- **Startup**, **Displays**, and **Command line** — secondary sections,
  collapsed until needed.

Closing the window keeps Moonglow running in the background so the schedule
and shortcuts stay active. Launch `moonglow --show-settings` (or simply
`moonglow` again) to bring it back, and use **Quit Moonglow** in the footer to
stop it completely. The autostart entry launches with `--hidden`.

A second invocation forwards to the running instance:

```bash
moonglow --show-settings
moonglow --set-brightness 70
moonglow --set-temp 4000
moonglow --auto off
moonglow --day-temp 6500 --night-temp 1300
moonglow --morning 06:00 09:00 --evening 18:00 21:00
moonglow --login on
moonglow --status
moonglow --quit
```

With no daemon running, the one-shot flags drive the backends directly (handy
headless or in scripts). `--headless` runs the schedule and shortcuts with no
window, and `--hidden` starts the window hidden. Relative steppers suit
desktop-level keyboard shortcuts:

```bash
moonglow --brighter   # or --dimmer (5%)
moonglow --warmer     # or --cooler (100 K, manual mode)
moonglow --brightness-up     # shortcut semantics: +5%
moonglow --brightness-down   # shortcut semantics: -5%
moonglow --temperature-up    # +100 K; turns automatic off first
moonglow --temperature-down  # -100 K; turns automatic off first
moonglow --status --json     # machine-readable, for scripts
```

Diagnostics go to `~/.local/state/moonglow/moonglow.log`
(`$XDG_STATE_HOME` when set), rotated past 2 MB to `moonglow.log.1`.
One-shot CLI output stays on the terminal; only the long-running instance logs.

## Global shortcuts

The Shortcuts card uses the best mechanism the session offers:

1. **GNOME custom keybindings** (`gsettings`) on GNOME, X11 or Wayland. The
   shortcuts also appear under Settings › Keyboard › Custom Shortcuts and
   survive quitting the app.
2. **XDG GlobalShortcuts portal** on KDE and recent wlroots desktops, where
   the compositor itself owns the key grab.
3. **X11 key grabs** on plain X11 sessions that are not GNOME.

Defaults are Ctrl+Alt+PageUp / Ctrl+Alt+PageDown for brightness and
Ctrl+Alt+] / Ctrl+Alt+[ for temperature, chosen because GNOME already owns the
Ctrl+Alt+arrow combinations for workspace switching. A shortcut needs at least one modifier, and
assigning a combination to one action removes it from any other.

Settings live at `~/.config/moonglow/settings.json` (same schema as macOS).
Start-at-login is a `~/.config/autostart` entry managed from the settings
window; a registration the desktop refuses is never recorded.

## Divergences from macOS

- Settings window is a native Adwaita preferences window (GTK4 + libadwaita),
  so it follows the desktop theme and runs as a real Wayland window. The API
  is capped at GTK 4.8 / libadwaita 1.2 to keep Debian 12 supported.
- Appearance always follows the desktop (no manual Light/Dark switch).
- Night-light backends clamp to their own floors (1000 K); X11/wlroots cover
  the full 900–6600 K range.
- DDC/CI drives every answering panel, not just the primary external display.
- Temperature shortcuts leave automatic mode without a confirmation dialog,
  because a global key press cannot reliably raise a modal behind another
  window.
- Closing the window hides it to the background; there is no tray icon.
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
- Shortcut registration on GNOME (X11 and Wayland), KDE Wayland, and X11;
  re-registration after editing a shortcut; clearing one.
- All five schedule states, including both transitions.
- Settings round trip through `~/.config/moonglow/settings.json`.
- Second invocation forwards; `--quit` stops the daemon; socket removed.
