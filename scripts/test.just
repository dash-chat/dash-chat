set working-directory := '..'

# run integration tests for the dashchat-node crate
dashchat-node:
    cd crates/dashchat-node && cargo nextest run

# run e2e tests
e2e:
    pnpm -F ./e2e-tests test

# run a single e2e test (e.g. just test e2e-one messaging)
e2e-one NAME:
    cd e2e-tests && SKIP_BUILD=1 pnpm wdio run wdio.conf.ts --spec specs/{{NAME}}.spec.ts

# run backwards compatibility tests against version tags (e.g. just test compat v0.10.0 v0.10.1)
compat *TAGS:
    bash e2e-tests/compat/run.sh {{TAGS}}
