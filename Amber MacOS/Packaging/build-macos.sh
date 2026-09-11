#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_dir="$(cd "$script_dir/.." && pwd)"
repo_dir="$(cd "$project_dir/.." && pwd)"
artifacts_dir="$repo_dir/artifacts"
release_dir="$repo_dir/release"
publish_dir="$artifacts_dir/publish-osx-arm64"
app_dir="$artifacts_dir/Amber.app"
dmg_staging_dir="$artifacts_dir/dmg"
iconset_dir="$artifacts_dir/Amber.iconset"
zip_path="$release_dir/Amber-macOS-arm64.zip"
dmg_path="$release_dir/Amber-macOS-arm64.dmg"

rm -rf "$publish_dir" "$app_dir" "$dmg_staging_dir" "$iconset_dir"
mkdir -p "$release_dir"
rm -f "$zip_path" "$dmg_path"

dotnet publish "$project_dir/Amber.MacOS.csproj" \
    --configuration Release \
    --runtime osx-arm64 \
    --self-contained true \
    -p:PublishSingleFile=true \
    --output "$publish_dir"

mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources" "$iconset_dir"
cp "$script_dir/Info.plist" "$app_dir/Contents/Info.plist"
cp -R "$publish_dir/." "$app_dir/Contents/MacOS/"
chmod +x "$app_dir/Contents/MacOS/Amber"

icon_source="$project_dir/Assets/Amber.png"
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
iconutil -c icns "$iconset_dir" -o "$app_dir/Contents/Resources/Amber.icns"

# Ad-hoc signing makes the native apphost and bundled libraries valid on Apple
# Silicon. It does not replace Developer ID signing or notarization.
codesign --force --deep --sign - "$app_dir"
codesign --verify --deep --strict "$app_dir"

ditto -c -k --sequesterRsrc --keepParent \
    "$app_dir" \
    "$zip_path"

mkdir -p "$dmg_staging_dir"
cp -R "$app_dir" "$dmg_staging_dir/"
ln -s /Applications "$dmg_staging_dir/Applications"

hdiutil create \
    -volname "Amber" \
    -srcfolder "$dmg_staging_dir" \
    -ov \
    -format UDZO \
    "$dmg_path"

echo "Created $zip_path"
echo "Created $dmg_path"