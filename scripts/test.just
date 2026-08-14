set working-directory := '..'

# run the full rust test suite with cargo nextest
[default]
default *ARGS:
    cargo nextest run {{ARGS}}

# run integration tests for the dashchat-node crate
node *ARGS:
    # config file is in .config/nextest.toml
    cd crates/dashchat-node && cargo nextest run {{ARGS}}

# alias for `just test node`
dashchat-node *ARGS:
    just test node {{ARGS}}

# run e2e tests (the harness builds what the agent combo needs first). Pass a
# spec name to run only that one (e.g. `just test e2e full-flow`); omit it to
# run the whole suite. Pass `--no-rerun` to skip WebdriverIO's spec retry on
# failures. The PLATFORMS env var lists the agents to launch as an unordered
# multiset of platforms (default `desktop,desktop`): desktop, android
# (physical device), android-emulator (booted automatically), or ios (connected
# iPhone). Set E2E_STRESS=1 to include the stress specs that otherwise skip
# themselves.
e2e NAME='' *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    cd e2e-tests
    spec=''
    # seeded with the config so it is never empty: bash 3.2 (macOS) treats
    # "${arr[@]}" on an empty array as an unbound variable under `set -u`.
    wdio_args=(wdio.conf.ts)
    export E2E_SPEC_FILE_RETRIES=1
    for arg in {{NAME}} {{ARGS}}; do
        case "$arg" in
            --no-rerun|--no-retry) export E2E_SPEC_FILE_RETRIES=0 ;;
            --*) wdio_args+=("$arg") ;;
            *) spec="$arg" ;;
        esac
    done
    if [[ -n "$spec" ]]; then
        wdio_args+=(--spec "specs/${spec}.spec.ts")
        # Naming a spec means running it whatever it is, including the stress
        # specs that otherwise skip themselves.
        export E2E_STRESS=1
    fi
    # wdio itself needs the androidDev shell's tools (adb, appium
    # chromedrivers, boot-emulator, SDK/NDK for the APK build); it layers
    # over the caller's default dev shell so tauri-driver/pnpm stay on PATH.
    if [[ "${PLATFORMS:-}" == *android* ]]; then
        exec nix develop "git+file:$(git rev-parse --show-toplevel)#androidDev" \
            --command pnpm wdio run "${wdio_args[@]}"
    else
        exec pnpm wdio run "${wdio_args[@]}"
    fi

# run backwards compatibility tests against version tags (e.g. just test compat v0.10.0 v0.10.1)
compat *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}
