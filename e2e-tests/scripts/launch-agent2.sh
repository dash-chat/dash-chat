#!/usr/bin/env bash
# Launch Tauri agent 2 for E2E testing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/e2e/agent-2"
export E2E_TEST=1
# Isolate WebKitGTK/libsoup data directories per instance to prevent
# SQLite "database is locked" errors on shared HSTS/cookie databases.
export XDG_DATA_HOME="$DATA_DIR/.local/share"
export XDG_CACHE_HOME="$DATA_DIR/.cache"
export XDG_CONFIG_HOME="$DATA_DIR/.config"
mkdir -p "$DATA_DIR"
exec "$ROOT/target/debug/dash-chat"
