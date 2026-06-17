set working-directory := '..'

# start two dash chat instances connected to a local mailbox server
[default]
dev:
    pnpm install
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
