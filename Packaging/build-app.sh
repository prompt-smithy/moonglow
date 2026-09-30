#!/usr/bin/env bash
set -euo pipefail

# Builds artifacts/Night Mode Controller.app plus a drag-to-Applications DMG and a ZIP.
#
# Replaces legacy-csharp/Amber MacOS/Packaging/build-macos.sh. The .NET script published a
# self-contained osx-arm64 apphost; SwiftPM produces a plain arm64 binary that this script
# wraps in the bundle.
#
# Distribution signing is not performed here. Ad-hoc signing makes the bundle locally
# runnable; a real release needs:
#   codesign --force --deep --options runtime --sign "Developer ID Application: ..." "$app_dir"
#   xcrun notarytool submit "$dmg_path" --keychain-profile <profile> --wait
#   xcrun stapler staple "$dmg_path"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
artifacts_dir="$repo_dir/artifacts"
release_dir="$repo_dir/release"
app_dir="$artifacts_dir/Night Mode Controller.app"
dmg_staging_dir="$artifacts_dir/dmg"
iconset_dir="$artifacts_dir/Night Mode Controller.iconset"
zip_path="$release_dir/Night-Mode-Controller-macOS-arm64.zip"
dmg_path="$release_dir/Night-Mode-Controller-macOS-arm64.dmg"
icon_source="$repo_dir/Resources/Amber.png"
configuration="${AMBER_CONFIGURATION:-release}"

# Fail early and explain, rather than emitting a half-built bundle. A broken active developer
# directory (mismatched SwiftPM, compiler, or SDK) makes every SwiftPM command fail, including
# this one. A Command Line Tools-only install is not sufficient on this machine.
if ! swift package --version >/dev/null 2>&1; then
    cat >&2 <<'MESSAGE'
error: the Swift toolchain could not run.

This usually means the active developer directory is broken, so SwiftPM, the compiler, and
the SDK disagree. Check with:

    xcode-select -p
    swift --version

The working setup is full Xcode. Install it, then run:

    sudo xcode-select -s /Applications/Xcode.app/Contents/Developer

MESSAGE
    exit 1
fi

cd "$repo_dir"
swift build --configuration "$configuration"
bin_dir="$(swift build --configuration "$configuration" --show-bin-path)"

# Remove the previous generated names as well as this build's outputs.
rm -rf \
    "$artifacts_dir/Amber.app" \
    "$artifacts_dir/Amber.iconset" \
    "$app_dir" \
    "$dmg_staging_dir" \
    "$iconset_dir"
mkdir -p "$release_dir" "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources" "$iconset_dir"
rm -f \
    "$release_dir/Amber-macOS-arm64.zip" \
    "$release_dir/Amber-macOS-arm64.dmg" \
    "$zip_path" \
    "$dmg_path"

cp "$bin_dir/Amber" "$app_dir/Contents/MacOS/Amber"
chmod +x "$app_dir/Contents/MacOS/Amber"
cp "$script_dir/Info.plist" "$app_dir/Contents/Info.plist"

sips -z 16 16 "$icon_source" --out "$iconset_dir/icon_16x16.png" >/dev/null
sips -z 32 32 "$icon_source" --out "$iconset_dir/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$icon_source" --out "$iconset_dir/icon_32x32.png" >/dev/null
sips -z 64 64 "$icon_source" --out "$iconset_dir/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$icon_source" --out "$iconset_dir/icon_128x128.png" >/dev/null
sips -z 256 256 "$icon_source" --out "$iconset_dir/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$icon_source" --out "$iconset_dir/icon_256x256.png" >/dev/null
sips -z 512 512 "$icon_source" --out "$iconset_dir/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$icon_source" --out "$iconset_dir/icon_512x512.png" >/dev/null
cp "$icon_source" "$iconset_dir/icon_512x512@2x.png"
iconutil -c icns "$iconset_dir" -o "$app_dir/Contents/Resources/NightModeController.icns"

# Ad-hoc signing makes the binary valid on Apple Silicon. It does not replace Developer ID
# signing or notarization.
codesign --force --deep --sign - "$app_dir"
codesign --verify --deep --strict "$app_dir"

ditto -c -k --sequesterRsrc --keepParent "$app_dir" "$zip_path"

mkdir -p "$dmg_staging_dir"
cp -R "$app_dir" "$dmg_staging_dir/"
ln -s /Applications "$dmg_staging_dir/Applications"

hdiutil create \
    -volname "Night Mode Controller" \
    -srcfolder "$dmg_staging_dir" \
    -ov \
    -format UDZO \
    "$dmg_path"

echo "Created $zip_path"
echo "Created $dmg_path"
