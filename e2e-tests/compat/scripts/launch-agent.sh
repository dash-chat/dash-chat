#!/usr/bin/env bash
# Launch a Tauri agent for compat E2E testing.
# Usage: launch-agent.sh <agent-number>
# Reads COMPAT_BINARY from environment.
set -euo pipefail

AGENT="${1:?Usage: launch-agent.sh <agent-number>}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
export DATA_DIR="$ROOT/.dbs/compat/agent-$AGENT"
# Old binaries check E2E_TEST env var at runtime to skip single-instance/updater plugins.
# Current binaries use the e2e-tests compile-time feature instead, but we keep this
# for backwards compatibility with older versions.
export E2E_TEST=1
mkdir -p "$DATA_DIR"
export MAILBOX_URL="${MAILBOX_URL:?MAILBOX_URL env var required}"
exec "${COMPAT_BINARY:?COMPAT_BINARY env var required}"
