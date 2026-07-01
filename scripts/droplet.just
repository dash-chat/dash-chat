set working-directory := '..'

# create a droplet running a server image from the github container registry (e.g. just droplet create mailbox-server 0.18.6 production)
create-mailbox-server version region='nyc1' size='s-1vcpu-2gb':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/mailbox-server:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/mailbox-server:{{version}} not found in the registry (run: just docker push mailbox-server)" >&2
        exit 1
    fi
    mailbox_domain="${MAILBOX_DOMAIN:?Set ENV=staging or ENV=production}"
    project="${DROPLET_PROJECT:?Set ENV=staging or ENV=production}"
    push_notifications_server_url="${PUSH_NOTIFICATIONS_SERVER_URL:?Set ENV=staging or ENV=production}"
    project_id=$(doctl projects list --format ID,Name --no-header | grep " $project$" | cut -d' ' -f1)
    server_args="--push-notifications-url $push_notifications_server_url"
    ssh_keys=$(doctl compute ssh-key list --format ID --no-header | paste -sd,)
    doctl compute droplet create "mailbox-server-{{replace(version, '.', '-')}}" \
        --image ubuntu-24-04-x64 \
        --region {{region}} \
        --size {{size}} \
        --project-id "$project_id" \
        --ssh-keys "$ssh_keys" \
        --user-data "#cloud-config
    packages:
      - docker.io
    runcmd:
      - mkdir -p /data /caddy
      - docker network create dash
      - docker run -d --name mailbox-server --restart unless-stopped --ulimit nofile=65536:65536 --network dash -v /data:/var/lib/mailbox-server ghcr.io/dash-chat/mailbox-server:{{version}} $server_args
      - docker run -d --name caddy --restart unless-stopped --network dash -p 80:80 -p 443:443 -v /caddy:/data caddy:2 caddy reverse-proxy --from $mailbox_domain --to mailbox-server:80" \
        --wait \
        --format Name,PublicIPv4,Region


# create a droplet running a server image from the github container registry (e.g. just droplet create mailbox-server 0.18.6 production)
create-push-notifications-server version region='nyc1' size='s-1vcpu-2gb':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/push-notifications-server:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/push-notifications-server:{{version}} not found in the registry (run: just docker push push-notifications-server)" >&2
        exit 1
    fi
    push_notifications_domain="${PUSH_NOTIFICATIONS_DOMAIN:?Set ENV=staging or ENV=production}"
    project="${DROPLET_PROJECT:?Set ENV=staging or ENV=production}"
    project_id=$(doctl projects list --format ID,Name --no-header | grep " $project$" | cut -d' ' -f1)
    ssh_keys=$(doctl compute ssh-key list --format ID --no-header | paste -sd,)
    doctl compute droplet create "push-notifications-server-{{replace(version, '.', '-')}}" \
        --image ubuntu-24-04-x64 \
        --region {{region}} \
        --size {{size}} \
        --project-id "$project_id" \
        --ssh-keys "$ssh_keys" \
        --user-data "#cloud-config
    packages:
      - docker.io
    runcmd:
      - mkdir -p /data /caddy
      - docker network create dash
      - docker run -d --name push-notifications-server --restart unless-stopped --ulimit nofile=65536:65536 --network dash -v /data:/var/lib/push-notifications-server ghcr.io/dash-chat/push-notifications-server:{{version}}
      - docker run -d --name caddy --restart unless-stopped --network dash -p 80:80 -p 443:443 -v /caddy:/data caddy:2 caddy reverse-proxy --from $push_notifications_domain --to push-notifications-server:80" \
        --wait \
        --format Name,PublicIPv4,Region

