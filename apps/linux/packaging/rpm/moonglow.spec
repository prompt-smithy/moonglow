Name:           moonglow
Version:        1.0.0
Release:        1%{?dist}
Summary:        Tray app for external-display DDC/CI brightness and scheduled colour temperature
License:        MIT AND Apache-2.0
URL:            https://github.com/prompt-smithy/moonglow
Source0:        %{url}/archive/refs/tags/linux-v%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  rustc
BuildRequires:  gcc
BuildRequires:  systemd
# DDC/CI debugging needs i2c-tools; the %post creates the i2c group itself.
Recommends:     i2c-tools
Requires(post): systemd-udev
ExclusiveArch:  x86_64 aarch64

%description
Moonglow drives external-monitor brightness over DDC/CI and
follows a morning/evening colour-temperature schedule. It lives in the
system tray with per-channel popups, scroll-to-adjust with on-screen
feedback, a settings window, and a full command-line interface.

%prep
%autosetup -n moonglow-linux-v%{version}

%build
# COPR builders have network access for the crates.io fetch. For Koji or
# offline builds, vendor the registry and add --offline.
cargo build --release --locked -p moonglow

%install
install -Dm755 target/release/moonglow %{buildroot}%{_bindir}/moonglow
install -Dm644 packaging/moonglow.desktop %{buildroot}%{_datadir}/applications/moonglow.desktop
install -Dm644 packaging/moonglow.metainfo.xml %{buildroot}%{_datadir}/metainfo/io.github.prompt-smithy.moonglow.metainfo.xml
for icon in moonglow-tray moonglow-brightness; do
    install -Dm644 packaging/icons/hicolor/scalable/apps/$icon.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/$icon.svg
done
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 packaging/icons/hicolor/${size}x${size}/apps/moonglow.png %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/moonglow.png
done
install -Dm644 packaging/51-moonglow-i2c.rules %{buildroot}%{_udevrulesdir}/51-moonglow-i2c.rules
install -Dm644 packaging/moonglow.1 %{buildroot}%{_mandir}/man1/moonglow.1
# NOTE: brp-compress gzips the man page automatically; do not pre-gzip.

%post
if ! getent group i2c >/dev/null; then
    groupadd -r i2c || :
fi
udevadm control --reload-rules || :
udevadm trigger --subsystem-match=i2c-dev --action=change || :
cat <<'EOF'

Moonglow: for DDC/CI brightness, join the i2c group and re-login:
  sudo usermod -aG i2c $USER
EOF

%files
%{_bindir}/moonglow
%{_datadir}/applications/moonglow.desktop
%{_datadir}/metainfo/io.github.prompt-smithy.moonglow.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/moonglow-tray.svg
%{_datadir}/icons/hicolor/scalable/apps/moonglow-brightness.svg
%{_datadir}/icons/hicolor/*/apps/moonglow.png
%{_udevrulesdir}/51-moonglow-i2c.rules
%{_mandir}/man1/moonglow.1*

%changelog
* Thu Oct 08 2026 Thomas Martin <tmmuttathara@gmail.com> - 1.0.0-1
- First stable Linux release.
