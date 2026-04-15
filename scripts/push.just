# run the push notifications dev server
server:
    #!/usr/bin/env bash
    set -euo pipefail
    test -f crates/push-notifications-server/service-account-key.json || { echo "ERROR: crates/push-notifications-server/service-account-key.json not found"; exit 1; }
    test -f src-tauri/gen/android/app/google-services.json || { echo "ERROR: src-tauri/gen/android/app/google-services.json not found"; exit 1; }
    cargo run -p push-notifications-server -- --db-path "$PWD/.dbs/dev/push-notifications-server/server.db" --service-account-key crates/push-notifications-server/service-account-key.json --addr 0.0.0.0:3001

# copy a Google service account key to the push notifications server
deploy-service-account-key file="crates/push-notifications-server/service-account-key.json":
    scp {{file}} root@push-notifications-server.dash-chat.garnix.me:/var/lib/push-notifications/service-account-key.json
