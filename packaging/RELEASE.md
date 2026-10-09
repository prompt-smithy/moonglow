# Release checklist (Linux)

Single source of truth for the version: `Cargo.toml`
`[workspace.package]`. Tags look like `linux-v1.0.0` (the `linux-`
prefix keeps Linux releases distinct from macOS tags in this repo).

## 1. Prepare

1. Bump `version` in `Cargo.toml`.
2. Add a `<release>` entry to
   `packaging/nightmodecontroller-linux.metainfo.xml`.
3. Add a `%changelog` entry to `packaging/rpm/nightmodecontroller-linux.spec`.
4. `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
   `cargo fmt --all --check` — all green.
5. Commit: `Release Linux vX.Y.Z`.

## 2. Tag + GitHub release

```bash
git tag linux-v1.0.0
git push origin linux-v1.0.0
```

Create the GitHub release from the tag and attach the `.deb` built in
step 3 (users on Mint/Pop/Zorin install the same file).

## 3. Debian / Ubuntu (.deb)

```bash
bash packaging/build-deb.sh   # → dist/nightmodecontroller-linux_*_amd64.deb
```

Smoke-test in a container or VM (never `--force` anything):

```bash
sudo dpkg -i dist/nightmodecontroller-linux_*_amd64.deb
sudo apt-get install -f          # pulls Recommends if wanted
nightmodecontroller-linux --version
man nightmodecontroller-linux        # renders
ls /usr/lib/udev/rules.d/51-nightmodecontroller-linux-i2c.rules
```

Then: `sudo usermod -aG i2c $USER`, log out/in, launch the app, check
tray + DDC + schedule.

## 4. Arch (AUR)

After the tag exists (the tarball URL must resolve):

```bash
cd packaging/arch
# bump pkgver/pkgrel in PKGBUILD, then:
updpkgsums            # or: makepkg -g >> PKGBUILD by hand
makepkg --printsrcinfo > .SRCINFO
makepkg -si           # local build + install test
```

Push `PKGBUILD`, `.SRCINFO`, `nightmodecontroller-linux.install` to the
AUR `nightmodecontroller-linux` repo (a separate checkout owned by the
maintainer account).

## 5. Fedora / openSUSE (COPR)

Follow `packaging/rpm/COPR.md` (`buildscm` from the new tag),
then install from the COPR on a Fedora VM and run the step-3 smoke
list (adapted: `rpm -ql`, `man`).

## 6. Post-release

- Record per-distro results in the test matrix (README or tracking
  issue): installer path, tray, popup, OSD, DDC, schedule, quit
  restore, log location.
- Kubuntu (KDE paths) and Debian stable (oldest stack) first, per the
  rollout plan.
