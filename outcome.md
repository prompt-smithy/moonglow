Status: ready-for-review

## Resolution

- Restored `docs/fable-review-verification.md` and `docs/plans/dual-brightness-control.md` from
  Git and checked the references in `system.md` and `fable_review.md`.
- Rewrapped the privacy policy, login-item and ad-hoc signing, and direct-download signing
  paragraphs in `apps/macos/README.md`. Verified the wording and links match the previous revision
  and all three paragraphs fit within 100 columns.

## Pending release work

- Sign the Mac App Store package, validate it with Apple, and test the Store build using the
  owner's credentials. Increase `CFBundleVersion` when preparing the next Store upload.
- Publish the updated privacy policy and verify its public URL for Settings and App Store Connect.
- Update the external Homebrew cask after the next notarized release.
