set working-directory := '..'

# Pin the iOS deployment target so cargo-built object files match the Xcode
# project (14.0). Setting this globally in .cargo/config.toml [env] breaks
# macOS builds because apple-sys/bindgen picks it up and targets iPhone.
export IPHONEOS_DEPLOYMENT_TARGET := "14.0"

# run the app in development mode
dev:
    cargo build --target aarch64-apple-ios
    pnpm tauri ios dev --host

build:
    cargo build --target aarch64-apple-ios --release
    pnpm tauri ios build

build-xcode:    
    cargo build --target aarch64-apple-ios --release
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
