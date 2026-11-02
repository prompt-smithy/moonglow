#!/usr/bin/env bash
set -euo pipefail

# Builds artifacts/Moonglow.app plus a drag-to-Applications DMG and a ZIP.
#
# SwiftPM produces separate architecture binaries; this script combines them into one universal
# app bundle for Intel and Apple Silicon Macs.
#
# If a Developer ID Application identity is installed, it is selected automatically. Set
# MOONGLOW_SIGNING_IDENTITY to override that choice, or set it to an empty value to force an
# ad-hoc local build. Notarization is opt-in through MOONGLOW_NOTARIZE=1.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
monorepo_dir="$(cd "$repo_dir/../.." && pwd)"
moonglow_version="${MOONGLOW_VERSION:-$(tr -d '[:space:]' < "$monorepo_dir/VERSION")}"
moonglow_build="${MOONGLOW_BUILD:-3}"
version_dir="$monorepo_dir/release/$moonglow_version"
artifacts_dir="${MOONGLOW_ARTIFACTS_DIR:-$repo_dir/artifacts}"
release_dir="${MOONGLOW_RELEASE_DIR:-$version_dir/macOs}"
build_dir="${MOONGLOW_BUILD_DIR:-$repo_dir/.build/universal}"
app_dir="$artifacts_dir/Moonglow.app"
dmg_staging_dir="$artifacts_dir/dmg"
iconset_dir="$artifacts_dir/Moonglow.iconset"
zip_path="$release_dir/Moonglow-macOS-universal.zip"
dmg_path="$release_dir/Moonglow-macOS-universal.dmg"
# Shared artwork lives at the repository root so every app consumes one master.
# The Mac icon follows the new SVG logo; the raster below is its 1024 px render.
icon_source="$repo_dir/../../resources/Moonglow-option-e-amber-noring.png"
configuration="${MOONGLOW_CONFIGURATION:-release}"
signing_identity="${MOONGLOW_SIGNING_IDENTITY:-}"
notarize="${MOONGLOW_NOTARIZE:-0}"
notary_keychain_profile="${MOONGLOW_NOTARY_KEYCHAIN_PROFILE:-}"
# Single source for the release version, stamped into the built plist below.
deployment_target="12.0"
arm64_triple="arm64-apple-macosx$deployment_target"
x86_64_triple="x86_64-apple-macosx$deployment_target"
arm64_scratch_path="$build_dir/arm64"
x86_64_scratch_path="$build_dir/x86_64"

if [ -z "${MOONGLOW_SIGNING_IDENTITY+x}" ]; then
    signing_identity="$(security find-identity -v -p codesigning 2>/dev/null \
        | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' \
        | head -n 1 || true)"
fi

if [ -n "$signing_identity" ]; then
    echo "Signing with $signing_identity"
else
    echo "No Developer ID Application identity found; using ad-hoc signing for local testing."
fi

sign_app() {
    local path="$1"
    if [ -n "$signing_identity" ]; then
        codesign --force --options runtime --timestamp --sign "$signing_identity" "$path"
    else
        codesign --force --sign - "$path"
    fi
}

sign_disk_image() {
    if [ -n "$signing_identity" ]; then
        codesign --force --timestamp --sign "$signing_identity" "$1"
    else
        codesign --force --sign - "$1"
    fi
}

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
mkdir -p "$version_dir/linux" "$version_dir/windows" "$version_dir/macOs"
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
# SwiftPM resource bundle (the menu-bar tray icon raster). Both architectures
# produce the same bundle, so one copy ships in the app. Without it the running
# app falls back to the drawn crescent in StatusIconFactory.
resource_bundle="$arm64_bin_dir/Moonglow_Moonglow.bundle"
if [ ! -d "$resource_bundle" ]; then
    echo "error: expected SwiftPM resource bundle at $resource_bundle" >&2
    exit 1
fi
ditto "$resource_bundle" "$app_dir/Contents/Resources/Moonglow_Moonglow.bundle"
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

# Sign the executable first, then the containing bundle. --deep is intentionally avoided for
# signing because it can hide incorrectly signed nested code.
sign_app "$app_dir/Contents/MacOS/Moonglow"
sign_app "$app_dir"
codesign --verify --strict --verbose=2 "$app_dir"

ditto -c -k --sequesterRsrc --keepParent "$app_dir" "$zip_path"

mkdir -p "$dmg_staging_dir"
cp -R "$app_dir" "$dmg_staging_dir/"
ln -s /Applications "$dmg_staging_dir/Applications"

hdiutil create -format UDZO -volname "Moonglow" -srcfolder "$dmg_staging_dir" "$dmg_path"

# The image is signed separately so it can be notarized as a release artifact.
sign_disk_image "$dmg_path"
codesign --verify --strict "$dmg_path"

if [ "$notarize" = "1" ]; then
    if [ -z "$signing_identity" ]; then
        echo "error: notarization requires a Developer ID Application identity" >&2
        exit 1
    fi
    if [ -z "$notary_keychain_profile" ]; then
        echo "error: set MOONGLOW_NOTARY_KEYCHAIN_PROFILE for notarization" >&2
        exit 1
    fi
    xcrun notarytool submit "$dmg_path" \
        --keychain-profile "$notary_keychain_profile" \
        --wait
    xcrun stapler staple "$dmg_path"
    xcrun stapler validate "$dmg_path"
fi
(cd "$release_dir" && shasum -a 256 "$(basename "$zip_path")" "$(basename "$dmg_path")" > SHA256SUMS)

echo "Created $zip_path"
echo "Created $dmg_path"
