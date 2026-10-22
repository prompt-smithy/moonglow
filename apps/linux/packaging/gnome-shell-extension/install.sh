#!/usr/bin/env bash
# Installs and enables the Moonglow GNOME Shell indicator.
# Requires `moonglow` on PATH and a running GNOME Shell.
set -euo pipefail

SRC="$(dirname "$0")/moonglow@moonglow.local"

if ! command -v moonglow >/dev/null; then
    echo "error: moonglow is not on PATH (install the .deb or cargo install first)" >&2
    exit 1
fi

# gnome-extensions install (from a zip) notifies a running Shell to rescan;
# a manual copy would sit unseen until the next login.
ZIP="$(mktemp -u).zip"
trap 'rm -f "$ZIP"' EXIT
(cd "$SRC" && python3 - "$ZIP" <<'EOF'
import sys
import zipfile
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as archive:
    for name in ["metadata.json", "extension.js", "stylesheet.css"]:
        archive.write(name)
EOF
)

gnome-extensions install --force "$ZIP"
gnome-extensions enable moonglow@moonglow.local
echo "Enabled. Look for the moon icon in the top bar."
