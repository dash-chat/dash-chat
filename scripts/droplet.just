set working-directory := '..'

# create a droplet running a server image from the github container registry (e.g. just droplet create mailbox-server 0.18.6)
create server version region='nyc1' size='s-1vcpu-2gb':
    #!/usr/bin/env bash
    set -euo pipefail
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

# build the mailbox server droplet image
mailbox: (build "mailbox-server")

# build the push notifications server droplet image
push-notifications: (build "push-notifications-server")

# build the digital ocean droplet image for a server and decompress it (e.g. just droplet build mailbox-server)
build server:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)
    image={{replace(server, "-server", "")}}-v${version}-do-image.qcow2
    nix build .#{{server}}-do-image --out-link .do-image-{{server}}
    cp .do-image-{{server}}/*.qcow2 "$image"
    chmod +w "$image"
    rm .do-image-{{server}}
    echo "Wrote $image"
