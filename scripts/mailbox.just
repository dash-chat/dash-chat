set working-directory := '..'

# run the mailbox dev server
server:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo run -p mailbox-server -- --db-path "$PWD/.dbs/dev/mailbox-server/mailbox.db" --addr "0.0.0.0:${MAILBOX_PORT:-3000}" --push-notifications-url "http://localhost:${PUSH_NOTIFICATIONS_SERVER_PORT:-3001}"
