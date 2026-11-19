Status: ready-for-review

## Resolution

- The sandboxed release uses one software brightness slider, including when settings restore a
  saved split layout. The hardware controls and alternate-build links are removed from that UI.
- The Store packager checks the provisioning profile and selects signing identities for the team.
  It verifies the app signature and three required entitlements on both architecture slices
  before packaging. App-only assembly preserves the direct ZIP and DMG.
- Packaging uses the validated `CFBundleVersion` from `Info.plist` by default. The direct release
  flow staples the app before making the Homebrew ZIP and DMG when notarization is enabled.
- `PRIVACY.md` covers all platforms, macOS Settings links to it, and `system.md` describes the
  build flavors and signing paths.

## Local verification

- Standard and sandboxed Swift test suites passed, including the split-control regression test.
- Both universal release apps passed strict ad-hoc signature checks. The sandboxed binary passed
  linkage and string checks for private hardware APIs, the gamma debug probe, and alternate-build
  links. Direct ZIP and DMG checksums passed before and after sandbox app assembly.
- Local fixtures exercised profile matching, legacy identity selection, and entitlement verification.
  The tray-icon bundle passed local signature and unsigned package payload checks.
- Packaging shell syntax, plists, and Git whitespace checks passed.

## Release verification

- Run `apps/macos/Packaging/build-mas.sh` with the owner's Store signing identities,
  `MOONGLOW_TEAM_ID`, and `MOONGLOW_MAS_PROVISION_PROFILE`. The script checks the signed installer
  with `pkgutil`. Validate it with `xcrun altool --validate-app` or Transporter, including Apple's
  acceptance of `Moonglow_Moonglow.bundle`. Store signing, Apple validation, and runtime testing
  of the Store build require the owner's credentials. Revisit the shared DDC log text and type name
  if App Review flags them.
- Increase `CFBundleVersion` against the latest accepted build when preparing a Store upload.
- Publish the updated policy to the public `main` branch and verify its URL for App Store Connect.
- Update the external Homebrew cask version and ZIP checksum after the next notarized release.
