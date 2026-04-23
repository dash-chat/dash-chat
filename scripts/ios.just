set working-directory := '..'

# run the app in development mode
dev:
    pnpm tauri ios dev

build:
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
