#!/usr/bin/env bash
# Build and deploy the mailbox-server as a systemd service.
#
# Usage:
#   ./scripts/deploy-mailbox.sh [--port PORT] [--db-path PATH]
#
# Workflow:
#   1. ssh into VPS
#   2. cd dash-chat && git pull
#   3. ./scripts/deploy-mailbox.sh
#
# The server will restart automatically on failure and start on boot.
# 
# Note: For the systemd user service to run when you're not logged in, you'll need to enable lingering on the VPS once: 
#
#     $ sudo loginctl enable-linger $USER.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

PORT="${MAILBOX_PORT:-3000}"
DB_PATH="${MAILBOX_DB_PATH:-$HOME/.local/share/mailbox-server/mailbox.redb}"

while [[ $# -gt 0 ]]; do
    case $1 in
        --port) PORT="$2"; shift 2 ;;
        --db-path) DB_PATH="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

SERVICE_NAME="mailbox-server"
UNIT_DIR="$HOME/.config/systemd/user"
UNIT_FILE="$UNIT_DIR/$SERVICE_NAME.service"
BINARY="$PROJECT_DIR/target/release/mailbox-server"

echo "==> Building mailbox-server (release)..."
cargo build --release -p mailbox-server --manifest-path "$PROJECT_DIR/Cargo.toml"

if [ ! -f "$UNIT_FILE" ]; then
    echo "==> Installing systemd user service..."
    mkdir -p "$UNIT_DIR"
    mkdir -p "$(dirname "$DB_PATH")"

    cat > "$UNIT_FILE" <<EOF
[Unit]
Description=Dash Chat Mailbox Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$BINARY --db-path $DB_PATH --addr 0.0.0.0:$PORT
Restart=on-failure
RestartSec=3
Environment=RUST_LOG=mailbox_server=info,tower_http=info
WorkingDirectory=$PROJECT_DIR

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload
    systemctl --user enable "$SERVICE_NAME"
else
    echo "==> Service unit already exists, skipping install."
fi

echo "==> Restarting service..."
systemctl --user restart "$SERVICE_NAME"

sleep 1
if systemctl --user is-active --quiet "$SERVICE_NAME"; then
    echo ""
    echo "==> mailbox-server is running on port $PORT"
    echo "    DB path: $DB_PATH"
    echo "    Logs:    journalctl --user -u $SERVICE_NAME -f"
    echo "    Status:  systemctl --user status $SERVICE_NAME"
    echo "    Stop:    systemctl --user stop $SERVICE_NAME"
else
    echo ""
    echo "==> Service failed to start. Check logs:"
    echo "    journalctl --user -u $SERVICE_NAME --no-pager -n 20"
    exit 1
fi
