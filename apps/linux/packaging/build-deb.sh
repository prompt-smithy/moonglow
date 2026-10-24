#!/usr/bin/env bash
# Builds the Ubuntu/Debian .deb by staging + dpkg-deb (no extra tooling).
#   bash packaging/build-deb.sh
# Output: dist/moonglow_<version>_amd64.deb
# Version comes from the workspace Cargo.toml (single source of truth).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

echo "Building release $VERSION..."
cargo build --release --locked -p moonglow

echo "Staging..."
install -Dm755 "target/release/moonglow" \
    "$STAGE/usr/bin/moonglow"
install -Dm644 "packaging/moonglow.desktop" \
    "$STAGE/usr/share/applications/moonglow.desktop"
install -Dm644 "packaging/moonglow.metainfo.xml" \
    "$STAGE/usr/share/metainfo/io.github.prompt-smithy.moonglow.metainfo.xml"
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "../../resources/linux/hicolor/${size}x${size}/apps/moonglow.png" \
        "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps/moonglow.png"
done
install -Dm644 "packaging/51-moonglow-i2c.rules" \
    "$STAGE/usr/lib/udev/rules.d/51-moonglow-i2c.rules"
gzip -kf "packaging/moonglow.1"
install -Dm644 "packaging/moonglow.1.gz" \
    "$STAGE/usr/share/man/man1/moonglow.1.gz"
rm -f "packaging/moonglow.1.gz"

mkdir -p "$STAGE/DEBIAN"
sed "s/@VERSION@/$VERSION/" packaging/debian/control > "$STAGE/DEBIAN/control"
install -m755 packaging/debian/postinst "$STAGE/DEBIAN/postinst"
install -m755 packaging/debian/postrm "$STAGE/DEBIAN/postrm"

mkdir -p dist
OUT="dist/moonglow_${VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT"
echo "Built: $OUT"
dpkg-deb --info "$OUT" | head -12
