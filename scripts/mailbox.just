set working-directory := '..'

# run the mailbox dev server
server:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo run -p mailbox-server -- --db-path "$PWD/.dbs/dev/mailbox-server/mailbox.db" --addr "0.0.0.0:${MAILBOX_PORT:-3000}" --push-notifications-url "http://localhost:${PUSH_NOTIFICATIONS_SERVER_PORT:-3001}"

# build the local message server for 64-bit raspberry pi (glibc 2.35+, i.e. raspberry pi os bookworm or later)
local-message-server-arm:
    #!/usr/bin/env bash
    set -euo pipefail
    # The dev shell's rpath points into the nix store, which doesn't exist on the pi
    unset CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS
    nix run --inputs-from . nixpkgs#cargo-zigbuild -- zigbuild --release -p mailbox-local-server --features vendored-openssl --target aarch64-unknown-linux-gnu.2.35
    cp target/aarch64-unknown-linux-gnu/release/mailbox-local-server target/local-message-server-aarch64-linux
    echo "built $PWD/target/local-message-server-aarch64-linux"
