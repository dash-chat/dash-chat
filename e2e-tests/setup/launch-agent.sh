#!/usr/bin/env bash
# Launch a Tauri agent for E2E testing.
# Usage: launch-agent.sh <agent-number>
set -euo pipefail

AGENT="${1:?Usage: launch-agent.sh <agent-number>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

# When multiple workers run in parallel, namespace agent dirs by worker id so
# they don't share state. E2E_WORKER_ID is set by wdio.conf.ts beforeSession.
WORKER_ID="${E2E_WORKER_ID:-default}"
export DATA_DIR="$ROOT/.dbs/e2e/worker-$WORKER_ID/agent-$AGENT"
export MAILBOX_URL="${MAILBOX_URL:?MAILBOX_URL env var required}"

# Disable AT-SPI accessibility bridge to prevent D-Bus contention.
export NO_AT_BRIDGE=1
export GTK_A11Y=none

# Disable the DMA-BUF renderer — it causes non-deterministic WebKitGTK freezes.
# See: https://github.com/tauri-apps/tauri/issues/13498
export WEBKIT_DISABLE_DMABUF_RENDERER=1

mkdir -p "$DATA_DIR"
# Redirect output to a log file the test runner tails and prints with an
# agent-specific prefix. Using `>` truncates per launch so retries start fresh.
exec "$ROOT/target/debug/dash-chat" > "$DATA_DIR/agent.log" 2>&1
