---
name: start-dev
description: Start the Dash Chat development environment (Tauri agents, UI dev server, mailbox server, stores watcher). Use this when you need to run and test the app.
user-invocable: false
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_wait_for, mcp__tauri__webview_interact, mcp__tauri__webview_keyboard, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Start Development Environment

Start all the processes needed to run Dash Chat locally. Do NOT use `pnpm start` or `mprocs` — they require an interactive TTY.

Use the Bash tool with `run_in_background: true` for each process so you get task IDs you can later stop with `TaskStop`.

## Step 1: Allocate free ports and temp directory

Run each of these as a **separate** Bash call (do NOT chain them with `&&`), and save the output values for use in later steps:

```bash
node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})"
```
Save the output as `UI_PORT`.

```bash
node -e "const s=require('net').createServer();s.listen(0,()=>{console.log(s.address().port);s.close()})"
```
Save the output as `MAILBOX_PORT`.

```bash
mktemp -d
```
Save the output as `DEV_DBS_PATH`.

Then substitute these values literally into the commands in later steps (do NOT use shell variable references like `$UI_PORT` — use the actual port numbers and paths).

## Step 2: Build stores

```bash
pnpm -F ./packages/stores build
```

## Step 3: Start all processes in the background

Launch each of these as a **separate background Bash task** (using `run_in_background: true`). Save the task IDs so you can stop them later.

1. **Stores watcher** (rebuilds on changes):
   ```bash
   pnpm -F ./packages/stores dev
   ```

2. **Mailbox server**:
   ```bash
   cargo run -p mailbox-server -- --db-path <DEV_DBS_PATH>/mailbox-server/mailbox.db --addr 0.0.0.0:<MAILBOX_PORT>
   ```

3. **UI dev server** (Vite):
   ```bash
   UI_PORT=<UI_PORT> pnpm -F ./ui start
   ```

4. **Tauri agent 1** (Rust backend — connects to the UI dev server):
   ```bash
   AGENT=1 MAILBOX_PORT=<MAILBOX_PORT> DEV_DBS_PATH=<DEV_DBS_PATH> pnpm tauri dev --config '{"build":{"devUrl":"http://localhost:<UI_PORT>"}}'
   ```

5. **Tauri agent 2** (optional — only start if testing p2p communication):
   ```bash
   AGENT=2 MAILBOX_PORT=<MAILBOX_PORT> DEV_DBS_PATH=<DEV_DBS_PATH> pnpm tauri dev --config '{"build":{"devUrl":"http://localhost:<UI_PORT>"}}'
   ```

Replace all `<UI_PORT>`, `<MAILBOX_PORT>`, and `<DEV_DBS_PATH>` placeholders with the actual values from Step 1.

## Step 4: Wait for the app to launch and discover MCP bridge ports

Use the `TaskOutput` tool (with `block: false`, `timeout: 10000`) to poll each agent's task output repeatedly until you see `MCP Bridge plugin initialized` in the output. Do NOT use the Bash tool for polling — use only `TaskOutput` and `Grep`/`Read` on the output files.

**IMPORTANT: The MCP bridge ports are dynamically assigned.** They default to 9223/9224 but will increment if those ports are already in use (e.g., 9225, 9226, etc.). You **must** extract the actual port from the log line:

```
[MCP][PLUGIN][INFO] MCP Bridge plugin initialized for 'Dash Chat' (studio.darksoil.dashchat) on 0.0.0.0:PORT
```

Use the `Grep` tool on the task output files to find the actual ports:
- Pattern: `MCP Bridge plugin initialized`
- Path: the output file for each agent task

## Step 5: Connect via Tauri MCP bridge

Use `driver_session` (start) with the **actual port** from Step 4 (not hardcoded 9223) to connect to the running Tauri instance, then use `webview_screenshot`, `webview_dom_snapshot`, and other Tauri MCP tools to inspect and interact with the UI.

When running **two agents**, each gets its own MCP bridge on a different port. Use the `appIdentifier` parameter (set to the port number) to target a specific agent:

```
driver_session(action: start, port: <agent1-port>)  # connects to Agent 1
driver_session(action: start, port: <agent2-port>)  # connects to Agent 2

webview_screenshot(appIdentifier: <agent1-port>)     # screenshot Agent 1
webview_screenshot(appIdentifier: <agent2-port>)     # screenshot Agent 2
```

Both sessions can be active simultaneously. The most recently connected app becomes the default (used when `appIdentifier` is omitted).

## Cleanup: Stop all dev processes

When done testing, stop the driver session and use `TaskStop` on each of the background task IDs you saved in Step 3.
