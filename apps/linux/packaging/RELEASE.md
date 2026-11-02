# Release checklist (Linux)

Release version: repository-root `VERSION`, kept aligned with `Cargo.toml`
`[workspace.package]`. Distributables are written to `release/<VERSION>/linux` at the repository root. Tags look like `v1.0.1` and build all three operating systems in the shared release workflow.

## 1. Prepare

1. Bump root `VERSION` and the native app version manifests together.
2. Add a `<release>` entry to
   `packaging/moonglow.metainfo.xml`.
3. Add a `%changelog` entry to `packaging/rpm/moonglow.spec`.
4. `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
   `cargo fmt --all --check` — all green.
5. Commit: `Release Linux vX.Y.Z`.

## 2. Tag + GitHub release

```bash
git tag v1.0.1
git push origin v1.0.1
```

Create the GitHub release from the tag and attach the `.deb` built in
step 3 (users on Mint/Pop/Zorin install the same file).

## 3. Debian / Ubuntu (.deb)

```bash
bash packaging/build-deb.sh   # → ../../release/<VERSION>/linux/moonglow_*_amd64.deb
```

Smoke-test in a container or VM (never `--force` anything):

```bash
sudo dpkg -i ../../release/<VERSION>/linux/moonglow_*_amd64.deb
sudo apt-get install -f          # pulls Recommends if wanted
moonglow --version
man moonglow        # renders
ls /usr/lib/udev/rules.d/51-moonglow-i2c.rules
```

Then: `sudo usermod -aG i2c $USER`, log out/in, launch the app, check
window + DDC + schedule + shortcuts.

## 4. Arch (AUR)

After the tag exists (the tarball URL must resolve):

```bash
cd packaging/arch
# bump pkgver/pkgrel in PKGBUILD, then:
updpkgsums            # or: makepkg -g >> PKGBUILD by hand
makepkg --printsrcinfo > .SRCINFO
makepkg -si           # local build + install test
```

Push `PKGBUILD`, `.SRCINFO`, `moonglow.install` to the
AUR `moonglow` repo (a separate checkout owned by the
maintainer account).

## 5. Fedora / openSUSE (COPR)

Follow `packaging/rpm/COPR.md` (`buildscm` from the new tag),
then install from the COPR on a Fedora VM and run the step-3 smoke
list (adapted: `rpm -ql`, `man`).

## 6. Post-release

- Record per-distro results in the test matrix (README or tracking
  issue): installer path, window, shortcuts, DDC, schedule, quit
  restore, log location.
- Kubuntu (KDE paths) and Debian stable (oldest stack) first, per the
  rollout plan.
