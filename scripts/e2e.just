set working-directory := '..'

# run the full e2e suite (the harness builds what the agent combo needs
# first). Set E2E_SPEC_FILE_RETRIES to rerun failing spec files (defaults to
# 0, or 1 in CI). The PLATFORMS env var lists the agents to launch as an
# unordered multiset of platforms (default `desktop,desktop`): desktop,
# android (physical device), android-emulator (booted automatically), or ios
# (connected iPhone). Set E2E_STRESS=1 to include the stress specs that
# otherwise skip themselves.
[default]
[doc('run the full e2e suite (the default recipe: plain `just e2e`)')]
suite *ARGS:
    cd e2e-tests && pnpm wdio run wdio.conf.ts {{ARGS}}

# a named spec runs whatever it is, including the stress specs that
# otherwise skip themselves.
[doc('run a single spec by name (e.g. `just e2e run full-flow`)')]
run NAME *ARGS:
    cd e2e-tests && E2E_STRESS=1 pnpm wdio run wdio.conf.ts --spec 'specs/{{NAME}}.spec.ts' {{ARGS}}

# run the whole suite including the stress and push specs that otherwise skip themselves
all *ARGS:
    cd e2e-tests && E2E_STRESS=1 E2E_PUSH=1 E2E_SPEC_FILE_RETRIES="${E2E_SPEC_FILE_RETRIES:-1}" pnpm wdio run wdio.conf.ts {{ARGS}}
