set working-directory := '..'

# Pin the iOS deployment target so cargo-built object files match the Xcode
# project (17.0). Setting this globally in .cargo/config.toml [env] breaks
# macOS builds because apple-sys/bindgen picks it up and targets iPhone.
# Must stay >= 17.0: Xcode 26 dropped the Swift back-deployment compatibility
# libraries (swiftCompatibility56/Concurrency/Packs), so a lower target makes
# swiftc emit __swift_FORCE_LOAD_$_swiftCompatibility* refs that fail to link.
export IPHONEOS_DEPLOYMENT_TARGET := "17.0"

# run the app in development mode. --host makes Tauri set TAURI_DEV_HOST and
# serve the dev URL on the LAN IP so a physical device can reach Vite + HMR.
dev:
    pnpm tauri ios dev --host

# run the app in an iPhone simulator
dev-sim device="iPhone 16":
    pnpm tauri ios dev "{{device}}"

# build the app
build:
    pnpm tauri ios build --debug

# open XCode to build the app
build-xcode:
    #!/usr/bin/env bash
    set -euo pipefail
    # Xcode compiles the Rust in its own environment (not this shell), so the
    # `.env.${ENV}` vars just loaded here won't reach the `option_env!` reads
    # baked in at compile time. Persist them into `.xcode.env.local`, which the
    # "Build Rust Code" phase sources. Drop any prior values for these vars
    # first so switching ENV doesn't leave stale exports; other lines (e.g.
    # manual PATH overrides) are left untouched.
    ENV_LOCAL="src-tauri/gen/apple/.xcode.env.local"
    touch "$ENV_LOCAL"
    sed -i '' '/^export MAILBOX_URL=/d; /^export PUSH_NOTIFICATIONS_SERVER_URL=/d' "$ENV_LOCAL"
    [ -n "${MAILBOX_URL:-}" ] && echo "export MAILBOX_URL=\"$MAILBOX_URL\"" >> "$ENV_LOCAL"
    [ -n "${PUSH_NOTIFICATIONS_SERVER_URL:-}" ] && echo "export PUSH_NOTIFICATIONS_SERVER_URL=\"$PUSH_NOTIFICATIONS_SERVER_URL\"" >> "$ENV_LOCAL"
    pnpm tauri ios build -o

# build and install the app on a USB-connected iPhone in production mode
install:
    #!/usr/bin/env bash
    set -euo pipefail
    pnpm tauri ios build
    xcrun devicectl list devices --json-output /tmp/dash-chat-ios-devices.json > /dev/null
    DEVICE_ID=$(jq -r '[.result.devices[] | select(.connectionProperties.tunnelState == "connected" and .connectionProperties.transportType == "wired")][0].identifier // ""' /tmp/dash-chat-ios-devices.json)
    if [ -z "$DEVICE_ID" ]; then
      echo "No USB-connected iPhone found" >&2
      exit 1
    fi
    xcrun devicectl device install app --device "$DEVICE_ID" 'src-tauri/gen/apple/build/Dash Chat.ipa'
