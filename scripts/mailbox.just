set working-directory := '..'

# run the mailbox dev server; MAILBOX_BLOB_THROTTLE=<bytes/s> caps how fast it
# serves blobs, to watch a slow download in a dev client
server:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo run -p mailbox-server -- --db-path "$PWD/.dbs/dev/mailbox-server/mailbox.db" --addr "0.0.0.0:${MAILBOX_PORT:-3000}" --push-notifications-url "http://localhost:${PUSH_NOTIFICATIONS_SERVER_PORT:-3001}" ${MAILBOX_BLOB_THROTTLE:+--blob-throttle "$MAILBOX_BLOB_THROTTLE"}
