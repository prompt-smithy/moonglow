Name:           nightmodecontroller-linux
Version:        1.0.0
Release:        1%{?dist}
Summary:        Tray app for external-display DDC/CI brightness and scheduled colour temperature
License:        MIT AND Apache-2.0
URL:            https://github.com/prompt-smithy/nightModeController_Linux
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
Night Mode Controller drives external-monitor brightness over DDC/CI and
follows a morning/evening colour-temperature schedule. It lives in the
system tray with per-channel popups, scroll-to-adjust with on-screen
feedback, a settings window, and a full command-line interface.

%prep
%autosetup -n nightModeController_Linux-linux-v%{version}

%build
# COPR builders have network access for the crates.io fetch. For Koji or
# offline builds, vendor the registry and add --offline.
cargo build --release --locked -p nightmodecontroller-linux

%install
install -Dm755 target/release/nightmodecontroller-linux %{buildroot}%{_bindir}/nightmodecontroller-linux
install -Dm644 packaging/nightmodecontroller-linux.desktop %{buildroot}%{_datadir}/applications/nightmodecontroller-linux.desktop
install -Dm644 packaging/nightmodecontroller-linux.metainfo.xml %{buildroot}%{_datadir}/metainfo/io.github.prompt-smithy.nightmodecontroller-linux.metainfo.xml
for icon in nightmodecontroller-linux nightmodecontroller-linux-brightness; do
    install -Dm644 packaging/icons/hicolor/scalable/apps/$icon.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/$icon.svg
done
install -Dm644 packaging/51-nightmodecontroller-linux-i2c.rules %{buildroot}%{_udevrulesdir}/51-nightmodecontroller-linux-i2c.rules
install -Dm644 packaging/nightmodecontroller-linux.1 %{buildroot}%{_mandir}/man1/nightmodecontroller-linux.1
# NOTE: brp-compress gzips the man page automatically; do not pre-gzip.

%post
if ! getent group i2c >/dev/null; then
    groupadd -r i2c || :
fi
udevadm control --reload-rules || :
udevadm trigger --subsystem-match=i2c-dev --action=change || :
cat <<'EOF'

Night Mode Controller: for DDC/CI brightness, join the i2c group and re-login:
  sudo usermod -aG i2c $USER
EOF

%files
%{_bindir}/nightmodecontroller-linux
%{_datadir}/applications/nightmodecontroller-linux.desktop
%{_datadir}/metainfo/io.github.prompt-smithy.nightmodecontroller-linux.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/nightmodecontroller-linux.svg
%{_datadir}/icons/hicolor/scalable/apps/nightmodecontroller-linux-brightness.svg
%{_udevrulesdir}/51-nightmodecontroller-linux-i2c.rules
%{_mandir}/man1/nightmodecontroller-linux.1*

%changelog
* Thu Oct 08 2026 Thomas Martin <tmmuttathara@gmail.com> - 1.0.0-1
- First stable Linux release.
