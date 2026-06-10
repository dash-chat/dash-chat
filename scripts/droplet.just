set working-directory := '..'

# build the mailbox server droplet image
mailbox: (build "mailbox-server")

# build the push notifications server droplet image
push-notifications: (build "push-notifications-server")

# build the digital ocean droplet image for a server and decompress it (e.g. just droplet build mailbox-server)
build server:
    #!/usr/bin/env bash
    set -euo pipefail
    nix build .#{{server}}-do-image --out-link .do-image-{{server}}
    gunzip -c .do-image-{{server}}/*.qcow2.gz > {{server}}-do-image.qcow2
    rm .do-image-{{server}}
    echo "Wrote {{server}}-do-image.qcow2"
