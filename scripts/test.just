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

# end-to-end tests (run, build, android-build)
mod e2e 'e2e.just'

# run backwards compatibility tests against version tags (e.g. just test compat v0.10.0 v0.10.1)
compat *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}
