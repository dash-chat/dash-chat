# Dash Chat

Dash Chat is an end to end encrypted messenger that works with internet, without internet and bridges between the two. As long as there is a way for devices to communicate with each other, Dash Chat works.

## 🚧 Dash Chat is in Pre-alpha 🚧

Dash Chat is in pre-alpha. We are currently rebuilding the application on top of [p2panda](https://github.com/p2panda/p2panda).

## Tech Stack

- Frontend: SvelteKit 5 with TypeScript, and [Konsta UI](https://konstaui.com/) as the component library
- Backend: Rust with [Tauri](https://tauri.app)
- P2P: [p2panda](https://p2panda.org) for peer-to-peer communication
- Build Tool: [Vite](https://vite.dev)
- Development: [just](https://just.systems/)

## Translate Dash Chat

Help translate Dash Chat! We use Weblate to crowdsource translations.

Please contact the Dash Chat team at hello [at] dashchat [dot] org if you're interested in becoming a reviewer for translation(s) in your language(s).

[Join the Dash Chat Weblate](https://hosted.weblate.org/projects/dash-chat).

## Developer setup

1. Install [Rust](https://rust-lang.org/tools/install/).
2. Install [pnpm](https://pnpm.io/).
3. Install the [Tauri pre-requisits](https://tauri.app/start/prerequisites/) for your platform.
4. Run `pnpm install`.

  OR

If you use nix, just use `nix develop` to enter the development shell and run `pnpm install` to install the `pnpm` dependencies.


### Running the app

To run the app, run this command:

```bash
just dev
```

This will spawn two instances of the tauri, forming a p2panda network of 2 nodes connected to a single mailbox server running locally. All data will be persisted to the .dev-dbs folder in the current folder. If you want to clean up the development databases, run:

```bash
just clean-dev
```
