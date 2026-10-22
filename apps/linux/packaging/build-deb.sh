#!/usr/bin/env bash
# Builds the Ubuntu/Debian .deb by staging + dpkg-deb (no extra tooling).
#   bash packaging/build-deb.sh
# Output: dist/nightmodecontroller-linux_<version>_amd64.deb
# Version comes from the workspace Cargo.toml (single source of truth).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

echo "Building release $VERSION..."
cargo build --release --locked -p nightmodecontroller-linux

echo "Staging..."
install -Dm755 "target/release/nightmodecontroller-linux" \
    "$STAGE/usr/bin/nightmodecontroller-linux"
install -Dm644 "packaging/nightmodecontroller-linux.desktop" \
    "$STAGE/usr/share/applications/nightmodecontroller-linux.desktop"
install -Dm644 "packaging/nightmodecontroller-linux.metainfo.xml" \
    "$STAGE/usr/share/metainfo/io.github.prompt-smithy.nightmodecontroller-linux.metainfo.xml"
for icon in nightmodecontroller-linux nightmodecontroller-linux-brightness; do
    install -Dm644 "packaging/icons/hicolor/scalable/apps/$icon.svg" \
        "$STAGE/usr/share/icons/hicolor/scalable/apps/$icon.svg"
done
install -Dm644 "packaging/51-nightmodecontroller-linux-i2c.rules" \
    "$STAGE/usr/lib/udev/rules.d/51-nightmodecontroller-linux-i2c.rules"
gzip -kf "packaging/nightmodecontroller-linux.1"
install -Dm644 "packaging/nightmodecontroller-linux.1.gz" \
    "$STAGE/usr/share/man/man1/nightmodecontroller-linux.1.gz"
rm -f "packaging/nightmodecontroller-linux.1.gz"

mkdir -p "$STAGE/DEBIAN"
sed "s/@VERSION@/$VERSION/" packaging/debian/control > "$STAGE/DEBIAN/control"
install -m755 packaging/debian/postinst "$STAGE/DEBIAN/postinst"
install -m755 packaging/debian/postrm "$STAGE/DEBIAN/postrm"

mkdir -p dist
OUT="dist/nightmodecontroller-linux_${VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT"
echo "Built: $OUT"
dpkg-deb --info "$OUT" | head -12
