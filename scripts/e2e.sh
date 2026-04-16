#!/usr/bin/env bash
set -euo pipefail

# Kill stale servers to avoid port conflicts
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
"$SCRIPT_DIR/kill-servers.sh"

npx playwright test "$@"
