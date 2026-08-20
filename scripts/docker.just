set working-directory := '..'

# build the mailbox server docker image and push it to the github container registry
mailbox: (build "mailbox-server")

# build the push notifications server docker image and push it to the github container registry
push-notifications: (build "push-notifications-server")

# build the local message server docker image (run it with `docker run --network host` so mDNS works)
local-message-server: (build "local-message-server")

# build the docker image for a server and load it into docker without pushing (e.g. just docker build mailbox-server)
build server:
    #!/usr/bin/env bash
    set -euo pipefail
    script=$(nix build .#{{server}}-docker --print-out-paths --no-link)
    "$script" | docker load

# push a server image to the github container registry (requires docker login ghcr.io) (e.g. just docker push mailbox-server 0.18.6)
push server version:
    docker push ghcr.io/dash-chat/{{server}}:{{version}}
