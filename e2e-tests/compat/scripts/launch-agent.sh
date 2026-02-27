#!/usr/bin/env bash
# Launch a Tauri agent for compat E2E testing.
# Usage: launch-agent.sh <agent-number>
# Reads COMPAT_BINARY from environment.
set -euo pipefail

AGENT="${1:?Usage: launch-agent.sh <agent-number>}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/compat/agent-$AGENT"
export E2E_TEST=1
mkdir -p "$DATA_DIR"
exec "${COMPAT_BINARY:?COMPAT_BINARY env var required}"
