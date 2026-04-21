set working-directory := '..'

# run the app in development mode
dev:
    pnpm tauri ios dev

build:
    pnpm tauri ios build -o
