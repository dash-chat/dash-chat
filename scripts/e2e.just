set working-directory := '..'

# run e2e tests (builds what the agent combo needs first). Pass a spec name to
# run only that one (e.g. `just test e2e run full-flow`); omit it to run the
# whole suite. Pass `--no-rerun` to skip WebdriverIO's spec retry on failures.
# AGENT_1/AGENT_2 env vars pick each agent's platform: desktop (default),
# android (physical device), or android-emulator (booted automatically), e.g.
# `AGENT_1=android just test e2e run send-messages` for phone + desktop.
[default]
run NAME='' *ARGS:
    bash ./scripts/run-e2e.sh {{ if NAME == '' { '-' } else { NAME } }} {{ARGS}}

# build the Tauri binary used by the e2e tests
build:
    pnpm tauri build --debug --no-bundle --features e2e-tests

# build the e2e APK (webview-debuggable, e2e feature, baked mailbox URL).
# 127.0.0.1:3200 is the phone's own loopback; the wdio config bridges it to
# the host's mailbox via `adb reverse tcp:3200 tcp:<host port>` per device.
# Debuginfo is dropped and symbols stripped (debug_assertions stays on, which
# is what makes the webview inspectable) — with them the APK is over 3GB.
android-build TARGETS='aarch64 armv7 x86_64':
    nix develop git+file:.#androidDev --command bash -c "MAILBOX_URL=http://127.0.0.1:3200 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_STRIP=symbols pnpm tauri android build --debug --apk --split-per-abi --features e2e-tests -t {{TARGETS}}"
