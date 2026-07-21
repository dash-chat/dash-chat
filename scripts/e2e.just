set working-directory := '..'

# run e2e tests (builds what the agent combo needs first). Pass a spec name to
# run only that one (e.g. `just test e2e run full-flow`); omit it to run the
# whole suite. Pass `--no-rerun` to skip WebdriverIO's spec retry on failures.
# The PLATFORMS env var lists the agents to launch as an unordered multiset of
# platforms (default `desktop,desktop`): desktop, android (physical device), or
# android-emulator (booted automatically), e.g.
# `PLATFORMS=android,desktop just test e2e run send-messages`.
[default]
run NAME='' *ARGS:
    bash ./scripts/run-e2e.sh {{ if NAME == '' { '-' } else { NAME } }} {{ARGS}}

# build the Tauri binary used by the e2e tests
build:
    pnpm tauri build --debug --no-bundle --features e2e-tests

