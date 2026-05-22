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

- **`webview_execute_js` timeout is ~10s**: The entire async chain in a single call must complete within ~10s. Keep individual `waitFor` timeouts to 8s max.
- **Animation frame after typing**: Svelte needs one animation frame after a synthetic `input` event before a follow-up click registers. The `typeInto` helper below awaits an animation frame already.
- **QR code uses `.value` property**: The `wa-qr-code` web component exposes the contact code as a JS `.value` property, NOT an HTML attribute. It may be `null` briefly after navigating — poll until it appears.
- **Chain everything**: Combine sequential steps into a single JS call. Maximize parallelism between agents.
- **No intermediate screenshots**: If a call returns successfully, the step worked. Only screenshot if something fails.

## `window.__test` (registered by `ui/tests/setup-utils.ts`)

The dev/e2e bridge exposes a deliberately small set of helpers — DOM-side primitives that can't be expressed as page-object clicks. There are **no high-level flow helpers** here; flows are scripted inline below.

| Helper | Purpose |
|---|---|
| `goto(path)` | SvelteKit `goto()` — programmatic navigation (escape hatch). |
| `setLocale(locale)` | Switch UI locale; triggers a full reload. |
| `tr(key)` | Resolve a paraglide message key in the current locale. |
| `hasText(selector, text)` | `querySelector(sel)?.textContent?.includes(text)` predicate. |
| `captureNextToastMessage(timeout?)` | Resolve with the next toast text. |
| `simulateUpdate(state)` | Fire the updater-banner state event. |
| `checkOverflow()` / `checkDarkMode()` / `checkRTL()` / `checkPage()` | Bulk DOM scans used by the review-checks spec. |

For everything else (clicking buttons, typing into inputs, waiting for elements), use the inline `waitFor` / `typeInto` snippets below or the canonical selectors in `e2e-tests/helpers/selectors.ts` and the page-object files under `e2e-tests/helpers/pages/`.

---

## Round 0: Start dev environment

Invoke the **start-dev** skill. Wait for READY, extract ports.

## Round 1: Connect (2 parallel calls)

```
driver_session(action: start, port: <agent1-port>)
driver_session(action: start, port: <agent2-port>)
```

## Round 2: Create profile + navigate + get code (2 parallel calls)

Run on **both agents simultaneously**. The inline `waitFor`/`typeInto` helpers stand in for the removed `window.__test.createProfile` / `navigateToAddContact` / `getContactCode` flows.

**Agent 1:**
```js
(async () => {
  const waitFor = async (sel, timeout = 8000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const el = document.querySelector(sel);
      if (el) return el;
      await new Promise(r => setTimeout(r, 50));
    }
    throw new Error(`waitFor timed out: ${sel}`);
  };
  const typeInto = async (sel, val) => {
    const el = await waitFor(sel);
    const proto = el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, val);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise(r => requestAnimationFrame(r));
  };
  await typeInto('[data-testid="create-profile-name"] input', 'Alice');
  await typeInto('[data-testid="create-profile-surname"] input', 'Test');
  (await waitFor('[data-testid="create-profile-create-btn"]')).click();
  await waitFor('[data-testid="all-chats-empty"]');
  (await waitFor('[data-testid="home-new-message-btn"]')).click();
  (await waitFor('[data-testid="new-message-add-contact"] a')).click();
  await waitFor('[data-testid="add-contact-code-input"]');
  const qr = await waitFor('wa-qr-code');
  const deadline = Date.now() + 8000;
  while (Date.now() < deadline) {
    if (qr.value) return qr.value;
    await new Promise(r => setTimeout(r, 50));
  }
  throw new Error('contact code never appeared');
})()
```

**Agent 2:** same as above with `'Bob'` / `'Tester'`.

Save returned strings as `agent1Code` and `agent2Code`.

## Round 3: Exchange contacts + send messages (2 parallel calls)

Run on **both agents simultaneously**. Each agent types the peer's code, waits for the direct-chat page, then types and sends a greeting.

**Agent 1:**
```js
(async () => {
  const waitFor = async (sel, timeout = 8000) => {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      const el = document.querySelector(sel);
      if (el) return el;
      await new Promise(r => setTimeout(r, 50));
    }
    throw new Error(`waitFor timed out: ${sel}`);
  };
  const typeInto = async (sel, val) => {
    const el = await waitFor(sel);
    const proto = el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, val);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise(r => requestAnimationFrame(r));
  };
  await typeInto('[data-testid="add-contact-code-input"] input', '<agent2Code>');
  await waitFor('[data-testid="direct-chat-page"]');
  await typeInto('[data-testid="message-input-textarea"] textarea', 'Hello from Alice!');
  (await waitFor('[data-testid="message-input-send"]')).click();
  return 'sent';
})()
```

**Agent 2:** same with `<agent1Code>` and `'Hello from Bob!'`.

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
