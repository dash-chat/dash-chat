#!/usr/bin/env bash
# Launch a Tauri agent for E2E testing.
# Usage: launch-agent.sh <agent-number>
set -euo pipefail

AGENT="${1:?Usage: launch-agent.sh <agent-number>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/e2e/agent-$AGENT"
export E2E_TEST=1
export MAILBOX_URL="${MAILBOX_URL:?MAILBOX_URL env var required}"
mkdir -p "$DATA_DIR"
exec "$ROOT/target/debug/dash-chat"
