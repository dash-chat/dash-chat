set working-directory := '..'

# start two dash chat instances connected to a local mailbox server
[default]
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    # Free ports keep concurrent `just dev` runs in different repo folders from
    # colliding. Data dirs live under this folder's .dbs/dev, so they're already
    # per-folder; the desktop agents reach the servers via these runtime URLs.
    export MAILBOX_PORT=$(pnpm --silent exec get-port)
    export PUSH_NOTIFICATIONS_SERVER_PORT=$(pnpm --silent exec get-port)
    echo "MAILBOX_PORT=$MAILBOX_PORT"
    echo "PUSH_NOTIFICATIONS_SERVER_PORT=$PUSH_NOTIFICATIONS_SERVER_PORT"
    pnpm exec mprocs

# start a tauri instance with its own Vite dev server on a free port, so several
# instances (agent1, agent2, …) can run side by side without colliding.
tauri:
    #!/usr/bin/env bash
    set -euo pipefail
    export UI_PORT=$(pnpm --silent exec get-port)
    pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$UI_PORT\"}}"

# clean all paths used by the dev environment
clean:
    rm -rf .dbs/dev
