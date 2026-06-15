set working-directory := '..'

# create a droplet running a server image from the github container registry (e.g. just droplet create mailbox-server 0.18.6 production)
create server version region='nyc1' size='s-1vcpu-2gb':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/{{server}}:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/{{server}}:{{version}} not found in the registry (run: just docker push {{server}})" >&2
        exit 1
    fi
    domain_suffix="${DOMAIN_SUFFIX:?Set ENV=staging or ENV=production}"
    project="${DROPLET_PROJECT:?Set ENV=staging or ENV=production}"
    project_id=$(doctl projects list --format ID,Name --no-header | grep " $project$" | cut -d' ' -f1)
    server_args=""
    if [ "{{server}}" = "mailbox-server" ]; then
        server_args="--push-notifications-url https://push-notifications.$domain_suffix"
    fi
    ssh_keys=$(doctl compute ssh-key list --format ID --no-header | paste -sd,)
    doctl compute droplet create "{{server}}-{{replace(version, '.', '-')}}" \
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
      - docker run -d --name {{server}} --restart unless-stopped --network dash -v /data:/var/lib/{{server}} ghcr.io/dash-chat/{{server}}:{{version}} $server_args
      - docker run -d --name caddy --restart unless-stopped --network dash -p 80:80 -p 443:443 -v /caddy:/data caddy:2 caddy reverse-proxy --from {{replace(server, '-server', '')}}.$domain_suffix --to {{server}}:80" \
        --wait \
        --format Name,PublicIPv4,Region

