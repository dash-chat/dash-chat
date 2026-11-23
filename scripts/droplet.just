set working-directory := '..'

# create a droplet running a server image from the github container registry (e.g. ENV=production just infra droplet create-mailbox-server 0.18.6)
create-mailbox-server version region='tor1' size='s-1vcpu-2gb': (_require-no-droplet "mailbox-server-" + replace(version, '.', '-'))
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/mailbox-server:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/mailbox-server:{{version}} not found in the registry (run: just infra docker push mailbox-server {{version}})" >&2
        exit 1
    fi
    mailbox_domain="${MAILBOX_DOMAIN:?Set ENV=staging or ENV=production or ENV=testing}"
    project="${DROPLET_PROJECT:?Set ENV=staging or ENV=production or ENV=testing}"
    push_notifications_server_url="${PUSH_NOTIFICATIONS_SERVER_URL:?Set ENV=staging or ENV=production or ENV=testing}"
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


# create a droplet running a server image from the github container registry (e.g. ENV=production just infra droplet create-push-notifications-server 0.18.6)
create-push-notifications-server version region='tor1' size='s-1vcpu-2gb': (_require-no-droplet "push-notifications-server-" + replace(version, '.', '-'))
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/push-notifications-server:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/push-notifications-server:{{version}} not found in the registry (run: just infra docker push push-notifications-server {{version}})" >&2
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

_require-no-droplet name:
    #!/usr/bin/env bash
    set -euo pipefail
    if doctl compute droplet list --format Name --no-header | grep -qx "{{name}}"; then
        echo "droplet {{name}} already exists" >&2
        exit 1
    fi

# print the public ip of a droplet (e.g. just infra droplet ip mailbox-server-0-18-6)
ip name:
    @doctl compute droplet list --format Name,PublicIPv4 --no-header | awk -v name="{{name}}" '$1 == name { print $2 }'

# short, so that clients follow a domain to a replacement droplet within a minute
a_record_ttl_seconds := '60'

# point a domain at an ip: creates or repoints the A record when digital ocean hosts the domain's zone, otherwise prints the record to set by hand
point-domain domain ip:
    #!/usr/bin/env bash
    set -euo pipefail
    domain="{{domain}}"
    zone=$(doctl compute domain list --format Domain --no-header | awk -v domain="$domain" 'substr(domain, length(domain) - length($1)) == "." $1' | head -n 1)
    if [ -z "$zone" ]; then
        echo "digital ocean does not host the zone of $domain: set its A record to {{ip}} by hand"
        exit 0
    fi
    record_name="${domain%.$zone}"
    record_id=$(doctl compute domain records list "$zone" --format ID,Type,Name --no-header | awk -v name="$record_name" '$2 == "A" && $3 == name { print $1 }')
    if [ -n "$record_id" ]; then
        doctl compute domain records update "$zone" --record-id "$record_id" --record-data {{ip}} --record-ttl {{a_record_ttl_seconds}}
    else
        doctl compute domain records create "$zone" --record-type A --record-name "$record_name" --record-data {{ip}} --record-ttl {{a_record_ttl_seconds}}
    fi

# wait until a domain serves https with a valid certificate, restarting caddy on its droplet once if it does not get there on its own
await-https domain ip:
    #!/usr/bin/env bash
    set -euo pipefail
    serves_https_within_5_minutes() {
        for _ in $(seq 30); do
            if curl --silent --output /dev/null --max-time 10 --resolve "{{domain}}:443:{{ip}}" "https://{{domain}}"; then
                return 0
            fi
            sleep 10
        done
        return 1
    }
    echo "waiting for https://{{domain}}"
    if serves_https_within_5_minutes; then
        exit 0
    fi
    # caddy backs off for minutes after a failed certificate request, and each restart spends one of let's encrypt's 5 failed validations per hour
    echo "restarting caddy on {{ip}}"
    ssh -o StrictHostKeyChecking=accept-new "root@{{ip}}" docker restart caddy
    if ! serves_https_within_5_minutes; then
        echo "https://{{domain}} still has no valid certificate: check that it resolves to {{ip}} and read 'docker logs caddy' on the droplet" >&2
        exit 1
    fi

