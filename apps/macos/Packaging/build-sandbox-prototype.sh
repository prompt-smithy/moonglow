#!/usr/bin/env bash
set -euo pipefail

# Build an isolated, software-only App Sandbox prototype for local feasibility testing.
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/.." && pwd)"

export MOONGLOW_APP_SANDBOXED=1
export MOONGLOW_SIGNING_IDENTITY="${MOONGLOW_SIGNING_IDENTITY-}"
export MOONGLOW_NOTARIZE=0
export MOONGLOW_BUNDLE_DISPLAY_NAME="${MOONGLOW_BUNDLE_DISPLAY_NAME:-Moonglow Sandbox Prototype}"
export MOONGLOW_BUNDLE_NAME="${MOONGLOW_BUNDLE_NAME:-Moonglow Sandbox Prototype}"
export MOONGLOW_BUNDLE_IDENTIFIER="${MOONGLOW_BUNDLE_IDENTIFIER:-com.prompt-smithy.MoonglowSandboxPrototype}"
export MOONGLOW_ARTIFACTS_DIR="${MOONGLOW_ARTIFACTS_DIR:-$repo_dir/artifacts/sandbox-prototype}"
export MOONGLOW_BUILD_DIR="${MOONGLOW_BUILD_DIR:-$repo_dir/.build/sandbox-prototype}"
export MOONGLOW_RELEASE_DIR="${MOONGLOW_RELEASE_DIR:-$repo_dir/../../release/sandbox-prototype/macOs}"

exec bash "$script_dir/build-app.sh"
