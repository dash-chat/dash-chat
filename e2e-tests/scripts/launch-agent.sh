#!/usr/bin/env bash
# Launch a Tauri agent for E2E testing.
# Usage: launch-agent.sh <agent-number>
set -euo pipefail

AGENT="${1:?Usage: launch-agent.sh <agent-number>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/e2e/agent-$AGENT"
export MAILBOX_URL="${MAILBOX_URL:?MAILBOX_URL env var required}"

# Disable AT-SPI accessibility bridge to prevent D-Bus contention.
export NO_AT_BRIDGE=1
export GTK_A11Y=none

# Disable the DMA-BUF renderer — it causes non-deterministic WebKitGTK freezes.
# See: https://github.com/tauri-apps/tauri/issues/13498
export WEBKIT_DISABLE_DMABUF_RENDERER=1

mkdir -p "$DATA_DIR"
exec "$ROOT/target/release/dash-chat"
