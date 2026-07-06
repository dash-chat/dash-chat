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

# build the Tauri binary used by the e2e tests
e2e-build:
    pnpm tauri build --debug --no-bundle --features e2e-tests

# run e2e tests (rebuilds the binary first). Pass a spec name to run only that
# one (e.g. `just test e2e full-flow`); omit it to run the whole suite. Pass
# `--no-rerun` to skip WebdriverIO's spec retry on failures.
e2e NAME='' *ARGS: e2e-build
    bash ./scripts/run-e2e.sh {{ if NAME == '' { '-' } else { NAME } }} {{ARGS}}

# run backwards compatibility tests against version tags (e.g. just test compat v0.10.0 v0.10.1)
compat *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}
