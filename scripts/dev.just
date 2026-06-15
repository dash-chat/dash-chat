set working-directory := '..'

# start two dash chat instances connected to a local mailbox server
[default]
dev:
    pnpm install
    pnpm exec mprocs

# clean all paths used by the dev environment
clean:
    rm -rf .dbs/dev
