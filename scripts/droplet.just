set working-directory := '..'

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
