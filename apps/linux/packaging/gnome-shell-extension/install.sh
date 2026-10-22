#!/usr/bin/env bash
# Installs and enables the Night Mode Controller GNOME Shell indicator.
# Requires `nightmodecontroller-linux` on PATH and a running GNOME Shell.
set -euo pipefail

SRC="$(dirname "$0")/nightmodecontroller-linux@amber.local"

if ! command -v nightmodecontroller-linux >/dev/null; then
    echo "error: nightmodecontroller-linux is not on PATH (install the .deb or cargo install first)" >&2
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
gnome-extensions enable nightmodecontroller-linux@amber.local
echo "Enabled. Look for the moon icon in the top bar."
