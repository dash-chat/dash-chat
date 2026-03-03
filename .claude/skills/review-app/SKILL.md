---
name: review-app
description: "Run a full app review — launches two instances, walks through every workflow, and checks all screens with iOS/Material themes and Farsi/German translations."
user-invocable: true
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Review App

A structured runbook for smoke-testing Dash Chat end-to-end. Uses `window.__test` helper functions (registered by `ui/tests/setup-utils.ts`) for automated checks, plus targeted screenshots for visual inspection.

Two parts:
1. **Automated checks** — `window.__test.visitAllPages()` navigates ~15 pages per combo, checking overflow/dark-mode/RTL at each stop. Covers all 16 combos in ~20 tool calls.
2. **Screenshot visual review** — LLM takes screenshots at key pages for qualitative visual inspection (alignment, spacing, "does it look right").

## Critical: Known tool limitations

These MCP tools **DO NOT WORK** in this app and must NEVER be used:

- **`webview_interact`** — fails with "resolveRef is not a function". Use `webview_execute_js` instead.
- **`webview_wait_for`** — fails with "resolveRef is not a function". Use `webview_execute_js` with polling instead.
- **`webview_keyboard`** — fails with "resolveRef is not a function". Use `webview_execute_js` instead.
- **`webview_dom_snapshot` with type `accessibility`** — fails with "aria-api library not loaded". Always use type `structure`.

## Critical: Konsta list item clicks

For Konsta `List` items (not `ListInput`), `data-testid` lands on the outer `<li>` but clicking the `<li>` does NOT navigate. The `window.__test.click()` helper handles this automatically (clicks `<a>` inside, falls back to element).

When writing inline JS, use the pattern:
```js
(document.querySelector('[data-testid="settings-profile-link"] a') || document.querySelector('[data-testid="settings-profile-link"]')).click()
```

## Critical: Theme resets on locale change

`window.__setLocale(...)` reloads the page, which **resets the theme back to Material** and **resets color scheme to light**. After a locale change:
1. Wait for `window.__test` to be re-registered (poll with `typeof window.__test !== 'undefined'`).
2. Re-apply theme via `theme-change` event if needed.
3. Re-apply layout via `set-wide-screen` event if needed.
4. For Farsi, set `document.documentElement.dir = 'rtl'`.

## Important: Navigation via UI elements only

**NEVER navigate by setting `window.location.href`**. All `window.__test` functions use click-based navigation through `data-testid` selectors, which tests real navigation paths.

The full selector registry is in `ui/tests/selectors.ts`.

---

## Layout modes: Desktop and Mobile

The app has two layouts:
- **Desktop**: Two-panel layout (sidebar 320px + content area).
- **Mobile**: Single-panel, full-width pages with back navigation.

```js
// Switch to mobile layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));

// Switch back to desktop layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));
```

The override persists across SvelteKit navigations but is **lost on full page reloads** (e.g., after `__setLocale()`).

---

## Combination matrix

All checks must cover **every combination** (16 total):

| # | Theme | Language | Layout | Color |
|---|-------|----------|--------|-------|
| 1 | Material | English | Desktop | Light |
| 2 | Material | English | Mobile | Light |
| 3 | iOS | English | Desktop | Light |
| 4 | iOS | English | Mobile | Light |
| 5 | Material | English | Desktop | Dark |
| 6 | Material | English | Mobile | Dark |
| 7 | iOS | English | Desktop | Dark |
| 8 | iOS | English | Mobile | Dark |
| 9 | Material | German | Desktop | Light |
| 10 | Material | German | Mobile | Light |
| 11 | iOS | German | Desktop | Light |
| 12 | iOS | German | Mobile | Light |
| 13 | Material | Farsi | Desktop | Light |
| 14 | Material | Farsi | Mobile | Light |
| 15 | iOS | Farsi | Desktop | Light |
| 16 | iOS | Farsi | Mobile | Light |

### Switching between combinations

Within a language (no reload needed):
```js
// Switch theme
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'ios' } }));

// Switch layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));

// Enable dark mode
window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: true }));

// Disable dark mode
window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: false }));
```

**Apply dark mode via `set-dark-mode` event.** This event updates both the CSS classes AND Konsta's `dark` prop (which controls iOS dark theme classes). Always dispatch `set-dark-mode` AFTER any `theme-change` event, as theme changes may cause re-renders.

---

## Phase 0: Start dev environment

1. Invoke the `start-dev` skill to launch both agents, the UI dev server, mailbox server, and stores watcher. **Start Agent 2** as well (it's needed for p2p testing).
2. Wait for Agent 1's task output to contain `MCP Bridge plugin initialized` — extract the actual port.
3. Wait for Agent 2's task output to contain `MCP Bridge plugin initialized` — extract the actual port.
4. Connect to Agent 1 via `driver_session` (start, port from step 2).

---

## Phase 1: Functional test + automated checks

This phase sets up the app state and runs automated checks across all 16 combos.

### 1.1 Setup (both agents)

**Agent 1:**
```js
await window.__test.createProfile('Alice', 'Test')
```

**Agent 2** (connect via `driver_session` with Agent 2's port):
```js
await window.__test.createProfile('Bob', 'Tester')
```

### 1.2 Contact exchange

**Agent 1:**
```js
await window.__test.navigateToAddContact()
```
Then extract code:
```js
window.__test.getContactCode()
```
Save the returned string as `agent1Code`.

**Agent 2:**
```js
await window.__test.navigateToAddContact()
```
Extract code, save as `agent2Code`. Then add Agent 1's code:
```js
await window.__test.addContact(agent1Code)
```

**Agent 1:**
```js
await window.__test.addContact(agent2Code)
```

### 1.3 Messaging

**Agent 1:**
```js
await window.__test.sendMessage('Hello from Alice!')
```

**Agent 2:**
```js
await window.__test.waitForMessage('Hello from Alice!')
await window.__test.sendMessage('Hello from Bob!')
```

**Agent 1:**
```js
await window.__test.waitForMessage('Hello from Bob!')
```

Then navigate Agent 1 back to home:
```js
window.__test.click('[data-testid="direct-chat-back"]')
await window.__test.waitFor('[data-testid="all-chats-list"]')
```

### 1.4 Run automated checks (all 16 combos)

For each combo, use `webview_execute_js` calls. **All remaining work is on Agent 1 only.**

**IMPORTANT:** `webview_execute_js` has a ~20s timeout. Use three separate calls per combo:
1. `visitProfilePages()` — home → settings → profile → edit-name/about/photo → add-contact → home
2. `visitOtherPages()` — home → settings → appearance/account → home → new-message → add-contact → home
3. `visitChatPages()` — home → direct-chat → chat-settings → home

Do NOT use `visitAllPages()` or `visitSettingsPages()` via MCP — they combine multiple chunks and will timeout.

#### English Light (4 combos)

For each of: Material Desktop, Material Mobile, iOS Desktop, iOS Mobile:

1. Switch combo:
```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'material' } }));
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));
```

2. Run checks (three separate `webview_execute_js` calls):
```js
await window.__test.visitProfilePages({ hasChat: true })
```
```js
await window.__test.visitOtherPages({ hasChat: true })
```
```js
await window.__test.visitChatPages({ hasChat: true })
```

3. Read `result.summary.totalIssues` and `result.pages` — record any issues.

#### English Dark (4 combos)

Enable dark mode:
```js
window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: true }))
```

For each combo, switch theme/layout, then run three calls with `checkDarkMode: true`:
```js
await window.__test.visitProfilePages({ hasChat: true, checkDarkMode: true })
```
```js
await window.__test.visitOtherPages({ hasChat: true, checkDarkMode: true })
```
```js
await window.__test.visitChatPages({ hasChat: true, checkDarkMode: true })
```

After all 4 combos, disable dark mode:
```js
window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: false }))
```

#### German (4 combos)

Switch locale (triggers reload):
```js
window.__setLocale('de-de')
```

Wait for reload — poll until `window.__test` is re-registered:
```js
typeof window.__test !== 'undefined'
```

For each combo, switch theme/layout, then run three calls:
```js
await window.__test.visitProfilePages({ hasChat: true })
```
```js
await window.__test.visitOtherPages({ hasChat: true })
```
```js
await window.__test.visitChatPages({ hasChat: true })
```

Check `navbarText` in results to verify translations.

#### Farsi RTL (4 combos)

Switch locale:
```js
window.__setLocale('fa-ir')
```

Wait for reload, then set RTL:
```js
document.documentElement.dir = 'rtl'
```

For each combo, switch theme/layout, re-apply `dir = 'rtl'`, then run three calls with `checkRTL: true`:
```js
await window.__test.visitProfilePages({ hasChat: true, checkRTL: true })
```
```js
await window.__test.visitOtherPages({ hasChat: true, checkRTL: true })
```
```js
await window.__test.visitChatPages({ hasChat: true, checkRTL: true })
```

Check `rtl` field in results to verify direction state.

---

## Phase 2: Screenshot visual review

Take screenshots at key pages for qualitative visual inspection. The automated checks in Phase 1 already verified overflow/dark-mode/RTL programmatically — screenshots catch issues JS can't detect (alignment, spacing, colors, "does it look right").

### English Light — 4 representative combos

For each of Material Desktop, Material Mobile, iOS Desktop, iOS Mobile:
1. Switch combo via events.
2. Navigate to each page using `window.__test.click()` and `window.__test.waitFor()`, take a `webview_screenshot` at each:
   - Home (`[data-testid="all-chats-list"]`)
   - Settings (`[data-testid="settings-profile-link"]`)
   - Profile (`[data-testid="profile-edit-name"]`)
   - Direct Chat (`[data-testid="direct-chat-messages"]`) — click first chat in list
   - Add Contact (`[data-testid="add-contact-back"]`) — via settings → profile → QR link
   - New Message (`[data-testid="new-message-add-contact"]`) — via new-message FAB/link
3. Navigate back to home after each sub-path.

~6 pages × 4 combos = ~24 screenshots.

### Dark mode spot check (2 combos × 2 pages)

Enable dark mode, screenshot Home + Direct Chat in:
- Material Desktop Dark
- iOS Desktop Dark

4 screenshots total.

### German + Farsi spot check (2 locales × 2 pages)

For each locale (switch via `__setLocale`, wait for reload):
- Screenshot Home + Settings in Material Desktop

4 screenshots total.

**Total: ~32 screenshots.**

Note any visual quality issues: alignment, spacing, colors, theme rendering, text legibility.

---

## Phase 3: Reset & cleanup

Reset to English + Material:
```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'material' } }));
window.__setLocale('en');
```

Stop all driver sessions via `driver_session` (action: `stop`).

Kill all background dev processes (Tauri agents, mailbox server, stores watcher, UI dev server) via `TaskStop` on each saved task ID from Phase 0.

---

## Phase 4: Report

Compile a summary of **all issues found**, categorized by:

- **Layout/overflow** — elements overflowing their containers, text truncation, horizontal scroll
- **Theme-specific** — issues only present in iOS or Material theme
- **Dark mode** — elements with wrong background/text colors in dark mode, hardcoded light colors, poor contrast
- **Locale-specific** — missing translations, German text overflow, RTL alignment problems
- **Visual quality** — alignment, spacing, colors, or rendering issues found in screenshots
- **Functional** — broken interactions, navigation failures, missing data

For each issue, include:
- The page/route where it was found
- The combo (theme/layout/color/locale)
- A description of the problem
- Whether it's a blocker or cosmetic

If no issues were found, report a clean bill of health.
