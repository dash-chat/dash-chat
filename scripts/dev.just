set working-directory := '..'

# start two dash chat instances connected to a local mailbox server
[default]
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    pnpm install
    # Free ports keep concurrent `just dev` runs in different repo folders from
    # colliding. Data dirs live under this folder's .dbs/dev, so they're already
    # per-folder; the desktop agents reach the servers via these runtime URLs.
    export UI_PORT=$(pnpm exec get-port)
    export MAILBOX_PORT=$(pnpm exec get-port)
    export PUSH_NOTIFICATIONS_SERVER_PORT=$(pnpm exec get-port)
    export MAILBOX_URL="http://localhost:$MAILBOX_PORT"
    export PUSH_NOTIFICATIONS_SERVER_URL="http://localhost:$PUSH_NOTIFICATIONS_SERVER_PORT"
    echo "UI_PORT=$UI_PORT"
    echo "MAILBOX_PORT=$MAILBOX_PORT"
    echo "PUSH_NOTIFICATIONS_SERVER_PORT=$PUSH_NOTIFICATIONS_SERVER_PORT"
    pnpm exec mprocs

# start a single tauri instance, pointing the app at UI_PORT if it is set
tauri:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "${UI_PORT:-}" ]; then
        pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$UI_PORT\"}}"
    else
        pnpm tauri dev
    fi

# clean all paths used by the dev environment
clean:
    rm -rf .dbs/dev
