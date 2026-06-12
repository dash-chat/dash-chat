set working-directory := '..'

# create a droplet running a server image from the github container registry (e.g. just droplet create mailbox-server 0.18.6)
create server version region='nyc1' size='s-1vcpu-2gb':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker manifest inspect ghcr.io/dash-chat/{{server}}:{{version}} > /dev/null; then
        echo "ghcr.io/dash-chat/{{server}}:{{version}} not found in the registry (run: just docker push {{server}})" >&2
        exit 1
    fi
    ssh_keys=$(doctl compute ssh-key list --format ID --no-header | paste -sd,)
    doctl compute droplet create "{{server}}-{{replace(version, '.', '-')}}" \
        --image ubuntu-24-04-x64 \
        --region {{region}} \
        --size {{size}} \
        --ssh-keys "$ssh_keys" \
        --user-data "#cloud-config
    packages:
      - docker.io
    runcmd:
      - mkdir -p /data
      - docker run -d --name {{server}} --restart unless-stopped -p 80:80 -v /data:/var/lib/{{server}} ghcr.io/dash-chat/{{server}}:{{version}}" \
        --wait \
        --format Name,PublicIPv4,Region

