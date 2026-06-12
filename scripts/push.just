set working-directory := '..'

# run the push notifications dev server
server:
    #!/usr/bin/env bash
    set -euo pipefail
    test -f crates/push-notifications-server/service-account-key.json || { echo "ERROR: crates/push-notifications-server/service-account-key.json not found"; exit 1; }
    cargo run -p push-notifications-server -- --db-path "$PWD/.dbs/dev/push-notifications-server/server.db" --service-account-key crates/push-notifications-server/service-account-key.json --addr 0.0.0.0:3001

# copy a Google service account key to the push notifications server and restart it
deploy-service-account-key env="production" file="crates/push-notifications-server/service-account-key.json":
    #!/usr/bin/env bash
    set -euo pipefail
    host=root@push-notifications.{{ if env == 'production' { 'darksoil.studio' } else { env + '.darksoil.studio' } }}
    scp {{file}} "$host":/data/service-account-key.json
    ssh "$host" docker restart push-notifications-server
