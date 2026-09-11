# macOS Release Files

The macOS GitHub Actions workflow generates these ad-hoc-signed Apple Silicon test packages in this folder:

- `Amber-macOS-arm64.dmg`
- `Amber-macOS-arm64.zip`

Download them from the workflow's `Amber-macOS-arm64` artifact. A genuine DMG is created by macOS `hdiutil`, so it cannot be generated locally on Windows.