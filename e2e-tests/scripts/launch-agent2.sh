#!/usr/bin/env bash
# Launch Tauri agent 2 for E2E testing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/e2e/agent-2"
export E2E_TEST=1
mkdir -p "$DATA_DIR"
exec "$ROOT/target/debug/dash-chat"
