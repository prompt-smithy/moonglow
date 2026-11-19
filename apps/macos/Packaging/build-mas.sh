#!/usr/bin/env bash
set -euo pipefail

# Builds the Mac App Store package: a software-only, sandboxed Moonglow.app signed with the
# Apple Distribution identity and wrapped in a .pkg signed with the Mac Installer Distribution identity.
#
# Required environment:
#   MOONGLOW_TEAM_ID                  Apple Developer Team ID (10 characters)
#   MOONGLOW_MAS_PROVISION_PROFILE    Path to the Mac App Store .provisionprofile for this bundle ID
#
# Optional:
#   MOONGLOW_MAS_APP_IDENTITY         Store application signing identity (auto-detected for the team if unset)
#   MOONGLOW_MAS_INSTALLER_IDENTITY   Store installer signing identity (auto-detected for the team if unset)
#   MOONGLOW_BUNDLE_IDENTIFIER        Defaults to com.prompt-smithy.Moonglow (must match the App Store Connect record)
#
# The app is built by build-app.sh in software-only mode, then re-signed here. Nothing is uploaded;
# validate and upload the .pkg with Transporter or `xcrun altool` afterwards.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"
monorepo_dir="$(cd "$repo_dir/../.." && pwd)"
moonglow_version="${MOONGLOW_VERSION:-$(tr -d '[:space:]' < "$monorepo_dir/VERSION")}"
bundle_identifier="${MOONGLOW_BUNDLE_IDENTIFIER:-com.prompt-smithy.Moonglow}"
artifacts_dir="$repo_dir/artifacts/appstore"
app_dir="$artifacts_dir/Moonglow.app"
release_dir="$monorepo_dir/release/$moonglow_version/macOs"
pkg_path="$release_dir/Moonglow-macOS-AppStore.pkg"
entitlements="$artifacts_dir/MoonglowMAS.entitlements"

fail() {
    echo "error: $*" >&2
    exit 1
}

plist_value() {
    printf '%s\n' "$1" | plutil -extract "$2" raw -expect "$3" -o - -
}

find_team_identity() {
    local names="$1"
    shift
    security find-identity -v "$@" 2>/dev/null | awk -F '"' -v names="$names" -v team="$team_id" '
        $2 ~ names && ($2 ~ (" \\(" team "\\)$") || $2 ~ (": " team "$")) { print $2; exit }
    ' || true
}

# --- Inputs -----------------------------------------------------------------

team_id="${MOONGLOW_TEAM_ID:-}"
[[ "$team_id" =~ ^[A-Z0-9]{10}$ ]] || fail "set MOONGLOW_TEAM_ID to your 10-character Apple Developer Team ID"
[[ "$bundle_identifier" =~ ^[A-Za-z0-9.-]+$ ]] || fail "invalid bundle identifier: $bundle_identifier"

profile="${MOONGLOW_MAS_PROVISION_PROFILE:-}"
[ -n "$profile" ] || fail "set MOONGLOW_MAS_PROVISION_PROFILE to the Mac App Store provisioning profile path"
[ -f "$profile" ] || fail "provisioning profile not found: $profile"

profile_plist="$(security cms -D -i "$profile")" || fail "could not decode provisioning profile: $profile"
profile_team="$(plist_value "$profile_plist" 'Entitlements.com\.apple\.developer\.team-identifier' string)" \
    || fail "provisioning profile has no team identifier"
[ "$profile_team" = "$team_id" ] || fail "provisioning profile does not match MOONGLOW_TEAM_ID"
application_id="$(plist_value "$profile_plist" 'Entitlements.com\.apple\.application-identifier' string)" \
    || fail "provisioning profile has no macOS application identifier"
app_id_prefix="${application_id%%.*}"
[[ "$app_id_prefix" =~ ^[A-Z0-9]{10}$ && "$application_id" = "$app_id_prefix.$bundle_identifier" ]] \
    || fail "provisioning profile does not match bundle identifier $bundle_identifier"

app_identity="${MOONGLOW_MAS_APP_IDENTITY:-}"
if [ -z "$app_identity" ]; then
    app_identity="$(find_team_identity '^(Apple Distribution|3rd Party Mac Developer Application): ' -p codesigning)"
fi
[ -n "$app_identity" ] || fail "no Store app signing identity for team $team_id (set MOONGLOW_MAS_APP_IDENTITY)"

installer_identity="${MOONGLOW_MAS_INSTALLER_IDENTITY:-}"
if [ -z "$installer_identity" ]; then
    installer_identity="$(find_team_identity '^(Mac Installer Distribution|3rd Party Mac Developer Installer): ')"
fi
[ -n "$installer_identity" ] || fail "no Store installer signing identity for team $team_id (set MOONGLOW_MAS_INSTALLER_IDENTITY)"

echo "App identity:       $app_identity"
echo "Installer identity: $installer_identity"
echo "Bundle identifier:  $bundle_identifier"

# --- Build the software-only app --------------------------------------------

MOONGLOW_APP_SANDBOXED=1 \
MOONGLOW_CONFIGURATION=release \
MOONGLOW_SIGNING_IDENTITY='' \
MOONGLOW_NOTARIZE=0 \
MOONGLOW_SKIP_PACKAGING=1 \
MOONGLOW_BUNDLE_IDENTIFIER="$bundle_identifier" \
MOONGLOW_BUNDLE_DISPLAY_NAME=Moonglow \
MOONGLOW_BUNDLE_NAME=Moonglow \
MOONGLOW_ARTIFACTS_DIR="$artifacts_dir" \
MOONGLOW_BUILD_DIR="$repo_dir/.build/appstore" \
MOONGLOW_RELEASE_DIR="$release_dir" \
bash "$script_dir/build-app.sh"

[ -d "$app_dir" ] || fail "build did not produce $app_dir"

# --- Provisioning profile and entitlements ----------------------------------

cp "$profile" "$app_dir/Contents/embedded.provisionprofile"

cp "$script_dir/AppStoreSandbox.entitlements" "$entitlements"
/usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string $application_id" "$entitlements"
/usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string $team_id" "$entitlements"

# --- Sign the app (after the profile is embedded, so the seal covers it) ----

codesign --force --timestamp --entitlements "$entitlements" --sign "$app_identity" "$app_dir"
codesign --verify --deep --strict --all-architectures --verbose=2 "$app_dir"
for arch in arm64 x86_64; do
    signed_entitlements="$(codesign --display --arch "$arch" --entitlements - --xml "$app_dir" 2>/dev/null)" \
        || fail "could not read signed entitlements for $arch"
    [ "$(plist_value "$signed_entitlements" 'com\.apple\.security\.app-sandbox' bool)" = true ] \
        || fail "App Sandbox entitlement missing for $arch"
    [ "$(plist_value "$signed_entitlements" 'com\.apple\.application-identifier' string)" = "$application_id" ] \
        || fail "signed application identifier does not match the profile for $arch"
    [ "$(plist_value "$signed_entitlements" 'com\.apple\.developer\.team-identifier' string)" = "$team_id" ] \
        || fail "signed team identifier does not match the profile for $arch"
    printf 'Entitlements for %s\n%s\n' "$arch" "$signed_entitlements"
done

# --- Package -----------------------------------------------------------------

mkdir -p "$release_dir"
rm -f "$pkg_path"
productbuild \
    --component "$app_dir" /Applications \
    --sign "$installer_identity" \
    "$pkg_path"
pkgutil --check-signature "$pkg_path"

echo "Created $pkg_path"
echo "Next: validate and upload it, for example:"
echo "  xcrun altool --validate-app -f \"$pkg_path\" -t macos --apiKey <KEY_ID> --apiIssuer <ISSUER_ID>"
echo "  xcrun altool --upload-app   -f \"$pkg_path\" -t macos --apiKey <KEY_ID> --apiIssuer <ISSUER_ID>"
