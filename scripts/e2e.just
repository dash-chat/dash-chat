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

