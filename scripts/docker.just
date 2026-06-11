set working-directory := '..'

# server must be either "mailbox-server" or "push-notificatons-server"

# build the docker image for a server and load it into docker without pushing (e.g. just docker build mailbox-server)
build server:
    #!/usr/bin/env bash
    set -euo pipefail
    script=$(nix build .#{{server}}-docker --print-out-paths --no-link)
    "$script" | docker load

# build a server image and push it to the github container registry (requires docker login ghcr.io)
push server: (build server)
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' src-tauri/tauri.conf.json | head -1)
    docker push ghcr.io/dash-chat/{{server}}:$version
