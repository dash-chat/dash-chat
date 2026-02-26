#!/usr/bin/env bash
# Launch a Tauri agent for E2E testing.
# Usage: launch-agent.sh <data-dir> [binary]
# binary defaults to the local debug build.
set -euo pipefail

DATA_DIR="${1:?Usage: launch-agent.sh <data-dir> [binary]}"
BINARY="${2:-$(cd "$(dirname "$0")/../.." && pwd)/target/debug/dash-chat}"

export DATA_DIR
export E2E_TEST=1
mkdir -p "$DATA_DIR"
exec "$BINARY" "${@:3}"
