#!/usr/bin/env bash
# Launch Tauri agent 1 for compat E2E testing.
# Reads COMPAT_BINARY and MAILBOX_URL from environment.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/compat/agent-1"
export E2E_TEST=1
mkdir -p "$DATA_DIR"
exec "${COMPAT_BINARY:?COMPAT_BINARY env var required}"
