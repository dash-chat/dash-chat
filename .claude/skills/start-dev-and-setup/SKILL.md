---
name: start-dev-and-setup
description: "Start the dev environment and set up two agents with profiles, exchanged contacts, and test messages."
user-invocable: true
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Start Dev Environment and Setup Test Users

Sets up two agents with profiles, exchanged contacts, and test messages in **3 rounds / 6 tool calls** after the dev environment is ready.

## Critical: Known tool limitations

Use `webview_execute_js` for ALL interactions. These tools do NOT work:

- **`webview_interact`** — fails with "resolveRef is not a function". Use `webview_execute_js` instead.
- **`webview_wait_for`** — fails with "resolveRef is not a function". Use `webview_execute_js` instead.
- **`webview_keyboard`** — fails with "resolveRef is not a function". Use `webview_execute_js` instead.
- **`webview_dom_snapshot` with type `accessibility`** — fails. Always use type `structure`.

## Key patterns

- **`webview_execute_js` timeout is ~10s**: The entire async chain in a single call must complete within ~10s. Keep individual `waitFor` timeouts to 8s max. Chain multiple steps only when the total is safely under 10s.
- **`nextTick` after `typeInto`**: Svelte needs one animation frame after synthetic input before a click will register. Always `await t.nextTick()` between `typeInto` and `click`. This is already built into `createProfile()` and `sendMessage()`.
- **QR code uses `.value` property**: The `wa-qr-code` web component exposes the contact code as a JS `.value` property, NOT an HTML attribute. `getContactCode()` may return `null` briefly after navigating — poll in a loop with 50ms interval.
- **Chain everything**: Combine sequential steps into a single JS call. Maximize parallelism between agents.
- **No intermediate screenshots**: If a call returns successfully, the step worked. Only screenshot if something fails.

## Test utilities

In dev mode, `window.__test` exposes:

- **Helpers**: `waitFor(sel, timeout?)`, `waitForText(sel, text, timeout?)`, `typeInto(sel, val)`, `click(sel)`, `nextTick()`
- **Flows**: `createProfile(name, surname)`, `navigateToAddContact()`, `getContactCode()`, `addContact(code)`, `sendMessage(text)`, `waitForMessage(text)`

Polling intervals: `waitFor` polls every 50ms, `waitForText` every 100ms. Default timeout is 15s.

Source: `ui/tests/` — `helpers.ts`, `setup-utils.ts`, `selectors.ts`, `flows/*.ts`, `pages/*.ts`

---

## Round 0: Start dev environment

Invoke the **start-dev** skill. Wait for READY, extract ports.

## Round 1: Connect (2 parallel calls)

```
driver_session(action: start, port: <agent1-port>)
driver_session(action: start, port: <agent2-port>)
```

## Round 2: Create profile + navigate + get code (2 parallel calls)

Run on **both agents simultaneously**. Each call creates the profile, waits for home, navigates to add-contact, polls for the QR code, and returns it:

**Agent 1:**
```js
(async () => {
  const t = window.__test;
  await t.createProfile('Alice', 'Test');
  await t.navigateToAddContact();
  while (!t.getContactCode()) await new Promise(r => setTimeout(r, 50));
  return t.getContactCode();
})()
```

**Agent 2:**
```js
(async () => {
  const t = window.__test;
  await t.createProfile('Bob', 'Tester');
  await t.navigateToAddContact();
  while (!t.getContactCode()) await new Promise(r => setTimeout(r, 50));
  return t.getContactCode();
})()
```

Save returned strings as `agent1Code` and `agent2Code`.

## Round 3: Exchange contacts + send messages (2 parallel calls)

Run on **both agents simultaneously**. Each adds the other's code and sends a message:

**Agent 1:**
```js
(async () => {
  const t = window.__test;
  await t.addContact('<agent2Code>');
  await t.sendMessage('Hello from Alice!');
  return 'sent';
})()
```

**Agent 2:**
```js
(async () => {
  const t = window.__test;
  await t.addContact('<agent1Code>');
  await t.sendMessage('Hello from Bob!');
  return 'sent';
})()
```

Both returning `'sent'` confirms: profiles created, contacts exchanged, messages delivered. No screenshot needed.

---

## Summary

| Round | Calls | Action |
|-------|-------|--------|
| 1 | 2 | `driver_session` × 2 |
| 2 | 2 | Create profile + navigate + get code × 2 |
| 3 | 2 | Add contact + send message × 2 |

**Total: 3 rounds, 6 tool calls** after dev environment is ready.

To clean up: `TaskStop` on the start-dev task ID, then `driver_session(action: stop)`.
