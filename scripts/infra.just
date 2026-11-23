set working-directory := '..'

# create digital ocean droplets and point domains at them
mod droplet 'droplet.just'

# build docker images for the mailbox, LAN mailbox and push notifications servers
mod docker 'docker.just'

# check that the environment's mailbox and push notifications servers answer requests at their domains (e.g. ENV=production just infra check)
check: check-mailbox-server check-push-notifications-server

# check that the environment's mailbox server answers its health route (e.g. ENV=production just infra check-mailbox-server)
check-mailbox-server:
    #!/usr/bin/env bash
    set -euo pipefail
    mailbox_url="${MAILBOX_URL:?Set ENV=staging or ENV=production or ENV=testing}"
    curl --fail --silent --show-error --max-time 10 "$mailbox_url/health" | jq --exit-status '.status == "ok"' > /dev/null
    echo "ok: mailbox server answers at $mailbox_url"

# check that the environment's push notifications server answers a request (e.g. ENV=production just infra check-push-notifications-server)
check-push-notifications-server:
    #!/usr/bin/env bash
    set -euo pipefail
    push_notifications_server_url="${PUSH_NOTIFICATIONS_SERVER_URL:?Set ENV=staging or ENV=production or ENV=testing}"
    # the push notifications server has no health route; notifying no topics reaches its database and sends nothing
    curl --fail --silent --show-error --max-time 10 --json '{"topics_to_notify":{}}' "$push_notifications_server_url/notify-topic"
    echo "ok: push notifications server answers at $push_notifications_server_url"

# build, publish and start the mailbox and push notifications servers for the current version, with their dns records and service account key, and wait until both serve https (e.g. ENV=production just infra create-all-servers-for-current-version)
create-all-servers-for-current-version:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(jq -r .version src-tauri/tauri.conf.json)
    for server in mailbox-server push-notifications-server; do
        if ! docker manifest inspect ghcr.io/dash-chat/$server:$version > /dev/null; then
            just infra docker build $server
            just infra docker push $server $version
        fi
    done
    just infra droplet create-push-notifications-server $version
    just infra droplet create-mailbox-server $version
    mailbox_ip=$(just infra droplet ip mailbox-server-${version//./-})
    push_notifications_ip=$(just infra droplet ip push-notifications-server-${version//./-})
    just infra droplet point-domain "$MAILBOX_DOMAIN" "$mailbox_ip"
    just infra droplet point-domain "$PUSH_NOTIFICATIONS_DOMAIN" "$push_notifications_ip"
    echo "waiting for the push notifications server to start on $push_notifications_ip"
    # droplet create returns before cloud-init has started the container that the key deploy restarts
    until ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=5 "root@$push_notifications_ip" docker inspect push-notifications-server > /dev/null 2>&1; do
        sleep 5
    done
    PUSH_NOTIFICATIONS_HOST="root@$push_notifications_ip" just push deploy-service-account-key
    just infra droplet await-https "$PUSH_NOTIFICATIONS_DOMAIN" "$push_notifications_ip"
    just infra droplet await-https "$MAILBOX_DOMAIN" "$mailbox_ip"
