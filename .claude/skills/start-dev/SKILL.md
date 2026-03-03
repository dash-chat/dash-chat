---
name: start-dev
description: Start the Dash Chat development environment (Tauri agents, UI dev server, mailbox server, stores watcher). Use this when you need to run and test the app.
user-invocable: false
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_wait_for, mcp__tauri__webview_interact, mcp__tauri__webview_keyboard, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Start Development Environment

Start all the processes needed to run Dash Chat locally. Do NOT use `pnpm start` or `mprocs` — they require an interactive TTY.

## Step 1: Run the start-dev script

Run the script as a **single background Bash task** (using `run_in_background: true`) and save the task ID:

```bash
# run_in_background: true
bash scripts/start-dev.sh
```

The script handles everything: allocating free ports, building stores, starting all processes (stores watcher, mailbox server, UI dev server, Tauri agents 1 and 2), and waiting for MCP bridges to initialize.

## Step 2: Wait for READY

Poll the task output using `TaskOutput` (with `block: false`, `timeout: 10000`) until you see `READY` in the output. If you see `ERROR:`, the startup failed — check the output for details.

## Step 3: Extract MCP bridge ports

Parse the task output for these KEY=VALUE lines:

- `AGENT1_MCP_PORT=<port>` — MCP bridge port for Agent 1
- `AGENT2_MCP_PORT=<port>` — MCP bridge port for Agent 2

Also available (for reference):
- `UI_PORT=<port>` — Vite dev server port
- `MAILBOX_URL=<url>` — Mailbox server URL
- `DEV_DBS_PATH=<path>` — Temp directory for databases and logs

## Step 4: Connect via Tauri MCP bridge

Use `driver_session` (start) with the **actual ports** from Step 3:

```
driver_session(action: start, port: <agent1-port>)  # connects to Agent 1
driver_session(action: start, port: <agent2-port>)  # connects to Agent 2
```

When running two agents, use the `appIdentifier` parameter (set to the port number) to target a specific agent:

```
webview_screenshot(appIdentifier: <agent1-port>)     # screenshot Agent 1
webview_screenshot(appIdentifier: <agent2-port>)     # screenshot Agent 2
```

Both sessions can be active simultaneously. The most recently connected app becomes the default (used when `appIdentifier` is omitted).

## Cleanup: Stop all dev processes

When done testing, stop the driver session and use `TaskStop` on the single task ID from Step 1. The script's cleanup trap will terminate all child processes automatically.
