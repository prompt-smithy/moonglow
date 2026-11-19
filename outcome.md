Status: ready-for-review

## Resolution

- Removed the historical review audit and superseded dual-brightness plan. The root README points
  to `system.md` for current architecture and `spec/` for the shared contract.
- Removed references to the deleted files from `system.md` and `fable_review.md`.

## Pending release work

- Sign the Mac App Store package, validate it with Apple, and test the Store build using the
  owner's credentials. Increase `CFBundleVersion` when preparing the next Store upload.
- Publish the updated privacy policy and verify its public URL for Settings and App Store Connect.
- Update the external Homebrew cask after the next notarized release.
