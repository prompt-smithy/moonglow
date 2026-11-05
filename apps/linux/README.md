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
  shipped udev rule. Packages install the rule automatically; portable and
  source builds need the one-time manual setup below.
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

### Enable DDC/CI brightness

The `.deb`, RPM, and AUR packages install the udev rule and create the `i2c`
group. Packaged installs still require adding your account to the group and
then logging out and back in. For a portable archive or a source build, install
the rule manually first.

From an extracted portable archive's version directory:

```bash
sudo install -Dm644 usr/lib/udev/rules.d/51-moonglow-i2c.rules \
  /usr/lib/udev/rules.d/51-moonglow-i2c.rules
```

Or, from the source repository root:

```bash
sudo install -Dm644 apps/linux/packaging/51-moonglow-i2c.rules \
  /usr/lib/udev/rules.d/51-moonglow-i2c.rules
```

Then run these commands for any install type:

```bash
sudo modprobe i2c-dev
# Ensure the group exists (the package normally creates it).
if ! getent group i2c >/dev/null; then
  sudo groupadd --system i2c
fi
# Add the current user; this is safe to repeat.
sudo usermod -aG i2c "$USER"
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=i2c-dev --action=change
```

Log out and back in so desktop-launched apps inherit the new group membership.
Then verify both the account and device permissions:

```bash
id -nG
ls -l /dev/i2c-*
moonglow --status
```

The nodes should show group `i2c` and permissions `crw-rw----`. If they still
show `root root` and `crw-------`, the udev rule is missing or has not been
applied. Restart Moonglow after fixing permissions so it reopens the I2C buses.
If there are no `/dev/i2c-*` nodes, check that `i2c-dev` is loaded and that
DDC/CI is enabled in the monitor's on-screen menu.

## Use

Launch `moonglow` (or the app launcher entry). The settings window opens with:

- **Brightness** — a slider for the hardware brightness of every answering
  DDC/CI panel.
- **Colour temperature** — a warm-to-cool slider, the automatic schedule
  switch, and the day/night range as one two-handle bar. The slider is
  disabled while Automatic is on; the schedule is driving the display.
- **Shortcuts** — global shortcuts for brightness, temperature, and showing
  Settings. Click a shortcut, press the new combination, and it is registered
  immediately. `Clear` removes one.
- **Schedule** — morning and evening transition windows.
- **Startup**, **Displays**, and **Command line** — secondary sections,
  collapsed until needed.

Settings are split into Display, Schedule, and Preferences pages. A fresh
manual launch opens Settings; after 10 seconds without keyboard or pointer
activity, the window hides while Moonglow keeps running in the background.
Launching `moonglow --show-settings` (or simply `moonglow` again) opens the
existing window. The configurable **Show settings** global shortcut does the
same. Use **Quit Moonglow** in Preferences to stop the app. The autostart entry
launches with `--hidden`, so Settings does not open at login.

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

Defaults are Ctrl+Alt+PageUp / Ctrl+Alt+PageDown for brightness,
Ctrl+Alt+] / Ctrl+Alt+[ for temperature, and Ctrl+Alt+M to show Settings.
These avoid GNOME's Ctrl+Alt+arrow workspace shortcuts. A shortcut needs at
least one modifier, and assigning a combination to one action removes it from
any other.

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
