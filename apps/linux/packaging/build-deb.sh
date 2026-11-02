#!/usr/bin/env bash
# Builds Linux distributables under the repository's release/<VERSION>/linux.
set -euo pipefail
repo_dir="$(cd "$(dirname "$0")/../../.." && pwd)"
exec python3 "$repo_dir/tools/build-release.py" --platform linux "$@"
