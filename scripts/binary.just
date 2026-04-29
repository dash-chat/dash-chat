set working-directory := '..'

# builds dash chat as a CLI binary
build:
    pnpm -F ./packages/stores build
    pnpm -F ./ui build
    cargo build --bins --release --locked --features tauri/custom-protocol,tauri/native-tls

# builds and runs dash chat as a binary
run: build
    ./target/release/dash-chat
