#!/usr/bin/env bash
# Start the Dash Chat development environment.
# Allocates ports, builds stores, starts all processes, and waits for MCP bridges.
# Output is structured KEY=VALUE lines that can be parsed by callers.
# The script stays alive until killed; on exit it cleans up all child processes.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_DIR"

# --- Step 1: Allocate free ports and temp directory ---

UI_PORT=$(node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})")
MAILBOX_PORT=$(node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})")
if command -v hostname &>/dev/null && hostname -I &>/dev/null; then
    LOCAL_IP=$(hostname -I | awk '{print $1}')
else
    LOCAL_IP=$(ipconfig getifaddr en0 2>/dev/null || echo "127.0.0.1")
fi
MAILBOX_URL="http://$LOCAL_IP:$MAILBOX_PORT"
DEV_DBS_PATH=$(mktemp -d)

echo "UI_PORT=$UI_PORT"
echo "MAILBOX_PORT=$MAILBOX_PORT"
echo "MAILBOX_URL=$MAILBOX_URL"
echo "DEV_DBS_PATH=$DEV_DBS_PATH"

LOGS_DIR="$DEV_DBS_PATH/logs"
mkdir -p "$LOGS_DIR" "$DEV_DBS_PATH/mailbox-server"

# --- Cleanup trap ---

PIDS=()

cleanup() {
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# --- Step 2: Start background processes ---

# Mailbox server
cargo run -p mailbox-server -- \
  --db-path "$DEV_DBS_PATH/mailbox-server/mailbox.db" \
  --addr "0.0.0.0:$MAILBOX_PORT" > "$LOGS_DIR/mailbox.log" 2>&1 &
PIDS+=($!)
echo "MAILBOX_PID=$!"

# Tauri agent 1 (beforeDevCommand runs pnpm dev: stores watcher + Vite)
export UI_PORT
DATA_DIR="$DEV_DBS_PATH/agent-1" MAILBOX_URL="$MAILBOX_URL" \
  pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$UI_PORT\"}}" \
  > "$LOGS_DIR/agent1.log" 2>&1 &
PIDS+=($!)
echo "AGENT1_PID=$!"

# Tauri agent 2 (skip beforeDevCommand since UI dev server is already running)
DATA_DIR="$DEV_DBS_PATH/agent-2" MAILBOX_URL="$MAILBOX_URL" \
  pnpm tauri dev --config "{\"build\":{\"beforeDevCommand\":\"\",\"devUrl\":\"http://localhost:$UI_PORT\"}}" \
  > "$LOGS_DIR/agent2.log" 2>&1 &
PIDS+=($!)
echo "AGENT2_PID=$!"

# --- Step 4: Wait for MCP bridge ports ---

echo "Waiting for MCP Bridge on Agent 1..."
AGENT1_MCP_PORT=""
for _ in $(seq 1 120); do
    if grep -q "MCP Bridge plugin initialized" "$LOGS_DIR/agent1.log" 2>/dev/null; then
        AGENT1_MCP_PORT=$(grep "MCP Bridge plugin initialized" "$LOGS_DIR/agent1.log" | grep -o '0\.0\.0\.0:[0-9]*' | head -1 | cut -d: -f2)
        echo "AGENT1_MCP_PORT=$AGENT1_MCP_PORT"
        break
    fi
    sleep 2
done

if [ -z "$AGENT1_MCP_PORT" ]; then
    echo "ERROR: Timed out waiting for Agent 1 MCP Bridge"
    exit 1
fi

echo "Waiting for MCP Bridge on Agent 2..."
AGENT2_MCP_PORT=""
for _ in $(seq 1 120); do
    if grep -q "MCP Bridge plugin initialized" "$LOGS_DIR/agent2.log" 2>/dev/null; then
        AGENT2_MCP_PORT=$(grep "MCP Bridge plugin initialized" "$LOGS_DIR/agent2.log" | grep -o '0\.0\.0\.0:[0-9]*' | head -1 | cut -d: -f2)
        echo "AGENT2_MCP_PORT=$AGENT2_MCP_PORT"
        break
    fi
    sleep 2
done

if [ -z "$AGENT2_MCP_PORT" ]; then
    echo "ERROR: Timed out waiting for Agent 2 MCP Bridge"
    exit 1
fi

echo "READY"

# Keep running until killed — child processes need this parent alive
wait
