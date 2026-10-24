#!/usr/bin/env bash
set -euo pipefail

# Builds artifacts/Moonglow.app plus a drag-to-Applications DMG and a ZIP.
#
# SwiftPM produces separate architecture binaries; this script combines them into one universal
# app bundle for Intel and Apple Silicon Macs.
#
# Distribution signing is not performed here. Ad-hoc signing makes the bundle locally
# runnable; a real release needs, in this order:
#   codesign --force --options runtime --sign "Developer ID Application: ..." "$app_dir/Contents/MacOS/Moonglow"
#   codesign --force --options runtime --sign "Developer ID Application: ..." "$app_dir"
#   ... build the DMG as below, then:
#   codesign --force --sign "Developer ID Application: ..." "$dmg_path"
#   xcrun notarytool submit "$dmg_path" --keychain-profile <profile> --wait
#   xcrun stapler staple "$dmg_path"
#
# The image itself must be signed: notarytool cannot staple an unsigned DMG.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
artifacts_dir="${MOONGLOW_ARTIFACTS_DIR:-$repo_dir/artifacts}"
release_dir="${MOONGLOW_RELEASE_DIR:-$repo_dir/release}"
build_dir="${MOONGLOW_BUILD_DIR:-$repo_dir/.build/universal}"
app_dir="$artifacts_dir/Moonglow.app"
dmg_staging_dir="$artifacts_dir/dmg"
iconset_dir="$artifacts_dir/Moonglow.iconset"
zip_path="$release_dir/Moonglow-macOS-universal.zip"
dmg_path="$release_dir/Moonglow-macOS-universal.dmg"
# Shared artwork lives at the repository root so every app consumes one master.
icon_source="$repo_dir/../../resources/Moonglow.png"
configuration="${MOONGLOW_CONFIGURATION:-release}"
# Single source for the release version, stamped into the built plist below.
moonglow_version="${MOONGLOW_VERSION:-0.2.0}"
moonglow_build="${MOONGLOW_BUILD:-2}"
deployment_target="12.0"
arm64_triple="arm64-apple-macosx$deployment_target"
x86_64_triple="x86_64-apple-macosx$deployment_target"
arm64_scratch_path="$build_dir/arm64"
x86_64_scratch_path="$build_dir/x86_64"

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
swift build --scratch-path "$arm64_scratch_path" --configuration "$configuration" --triple "$arm64_triple"
arm64_bin_dir="$(swift build --scratch-path "$arm64_scratch_path" --configuration "$configuration" --triple "$arm64_triple" --show-bin-path)"
swift build --scratch-path "$x86_64_scratch_path" --configuration "$configuration" --triple "$x86_64_triple"
x86_64_bin_dir="$(swift build --scratch-path "$x86_64_scratch_path" --configuration "$configuration" --triple "$x86_64_triple" --show-bin-path)"

# Remove the previous generated names as well as this build's outputs.
rm -rf \
    "$artifacts_dir/Night Mode Controller.app" \
    "$artifacts_dir/Night Mode Controller.iconset" \
    "$artifacts_dir/Amber.app" \
    "$artifacts_dir/Amber.iconset" \
    "$app_dir" \
    "$dmg_staging_dir" \
    "$iconset_dir"
mkdir -p "$release_dir" "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources" "$iconset_dir"
rm -f \
    "$release_dir/Night-Mode-Controller-macOS-universal.zip" \
    "$release_dir/Night-Mode-Controller-macOS-universal.dmg" \
    "$release_dir/Amber-macOS-arm64.zip" \
    "$release_dir/Amber-macOS-arm64.dmg" \
    "$zip_path" \
    "$dmg_path"

lipo -create \
    "$arm64_bin_dir/Moonglow" \
    "$x86_64_bin_dir/Moonglow" \
    -output "$app_dir/Contents/MacOS/Moonglow"
chmod +x "$app_dir/Contents/MacOS/Moonglow"
cp "$script_dir/Info.plist" "$app_dir/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "$moonglow_version" "$app_dir/Contents/Info.plist"
plutil -replace CFBundleVersion -string "$moonglow_build" "$app_dir/Contents/Info.plist"
plutil -insert MoonglowInstallationID -string "$moonglow_version ($moonglow_build)" "$app_dir/Contents/Info.plist"

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
iconutil -c icns "$iconset_dir" -o "$app_dir/Contents/Resources/Moonglow.icns"

# Ad-hoc signing is for local testing only. It does not replace Developer ID signing or
# notarization. Each level is signed explicitly: --deep is deprecated for signing.
codesign --force --sign - "$app_dir/Contents/MacOS/Moonglow"
codesign --force --sign - "$app_dir"
codesign --verify --strict "$app_dir"

ditto -c -k --sequesterRsrc --keepParent "$app_dir" "$zip_path"

mkdir -p "$dmg_staging_dir"
cp -R "$app_dir" "$dmg_staging_dir/"
ln -s /Applications "$dmg_staging_dir/Applications"

diskutil image create from \
    --format UDZO \
    --volumeName "Moonglow" \
    "$dmg_staging_dir" \
    "$dmg_path"

# The image is signed like the app: an unsigned DMG can be neither submitted nor stapled.
codesign --force --sign - "$dmg_path"
codesign --verify --strict "$dmg_path"

echo "Created $zip_path"
echo "Created $dmg_path"
