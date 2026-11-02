# Fedora / openSUSE via COPR

No `rpmbuild` runs in this repo's dev loop, so the spec is verified by
building in COPR (which also publishes the repo users install from).

## One-time setup

1. Install the CLI: `sudo dnf install copr-cli` and run
   `copr-cli whoami` (authorizes against your Fedora account).
2. Create the project (once):
   `copr-cli create moonglow --chroot fedora-42-x86_64 --chroot fedora-42-aarch64`
   (adjust the Fedora release to current; add `opensuse-leap-*` chroots
   if wanted — the spec avoids Fedora-only macros).

## Per release (after tagging `linux-vX.Y.Z`)

Build directly from git — COPR fetches the tag tarball itself:

```bash
copr-cli buildscm moonglow \
    --clone-url https://github.com/prompt-smithy/moonglow.git \
    --commit v1.0.1 \
    --spec packaging/rpm/moonglow.spec
```

Watch the build page it prints. First builds take a while (full cargo
fetch + release compile); later ones reuse nothing server-side, so
expect ~10 min per arch.

## Verify

On a Fedora VM/container with the COPR enabled:

```bash
sudo dnf copr enable <you>/moonglow
sudo dnf install moonglow
rpm -ql moonglow
moonglow --version
```

## Notes

- COPR builders have network access, so the plain
  `cargo build --release --locked` works. Official Fedora (Koji)
  forbids network at build time and would need a vendored registry —
  out of scope for v1.
- The `%post` scriptlet creates the `i2c` group when missing and
  reloads udev; users still join the group themselves
  (`sudo usermod -aG i2c $USER` + re-login).
