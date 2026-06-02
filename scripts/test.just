set working-directory := '..'

# run integration tests for the dashchat-node crate
dashchat-node:
    cd crates/dashchat-node && cargo nextest run --success-output never --failure-output immediate --final-status-level slow

# build the Tauri binary used by the e2e tests
e2e-build:
    pnpm tauri build --debug --no-bundle --features e2e-tests

# run e2e tests (rebuilds the binary first). Pass a spec name to run only that
# one (e.g. `just test e2e full-flow`); omit it to run the whole suite.
e2e NAME='': e2e-build
    cd e2e-tests && pnpm wdio run wdio.conf.ts {{ if NAME == '' { '' } else { '--spec specs/' + NAME + '.spec.ts' } }}

# run backwards compatibility tests against version tags (e.g. just test compat v0.10.0 v0.10.1)
compat *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}
