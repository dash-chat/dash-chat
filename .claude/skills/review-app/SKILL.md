---
name: review-app
description: "Run a full app review — launches two instances, walks through every workflow, and checks all screens with iOS/Material themes and Farsi/German translations."
user-invocable: true
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Review App

A structured runbook for smoke-testing Dash Chat end-to-end. Launches two app instances, exercises the full user workflow, and visually inspects every screen across Konsta UI themes (iOS + Material) and locales (English, German, Farsi).

## Critical: Known tool limitations

These MCP tools **DO NOT WORK** in this app and must NEVER be used:

- **`webview_interact`** — fails with "resolveRef is not a function". Use `webview_execute_js` with `document.querySelector(...).click()` instead.
- **`webview_wait_for`** — fails with "resolveRef is not a function". Use `webview_execute_js` with polling instead.
- **`webview_keyboard`** — fails with "resolveRef is not a function". Use `webview_execute_js` with the native value setter pattern instead.
- **`webview_dom_snapshot` with type `accessibility`** — fails with "aria-api library not loaded". Always use type `structure`.

## Critical: Speed and batching

**Move fast.** Do not pause between pages or wait for user confirmation. Batch multiple operations into single `webview_execute_js` calls wherever possible. For example, combine clicking a button + waiting for the next page + running overflow checks into a single JS script. Make parallel tool calls (screenshot + DOM snapshot + overflow check) in a single message.

**IMPORTANT: Keep batched JS scripts short.** `webview_execute_js` has an execution timeout (~20-30s). Deeply nested callback chains that navigate 5+ pages in a single script WILL time out. Limit each script to navigating at most 3-4 pages. If you need to visit more pages, split into multiple `webview_execute_js` calls.

## Critical: Konsta list item clicks

For Konsta `List` items (not `ListInput`), `data-testid` lands on the outer `<li>` but clicking the `<li>` does NOT navigate. You **must click the `<a>` inside it**:

```js
// WRONG — clicking <li> does nothing:
document.querySelector('[data-testid="settings-profile-link"]').click()

// CORRECT — click the <a> inside:
(document.querySelector('[data-testid="settings-profile-link"] a') || document.querySelector('[data-testid="settings-profile-link"]')).click()
```

This applies to: `settings-profile-link`, `settings-account-link`, `profile-edit-name`, `profile-edit-about`, `profile-edit-photo`, and any other list item that navigates to a new page. Use the `|| document.querySelector(...)` fallback pattern so it still works if the structure changes.

## Critical: Screenshots are stale

`webview_screenshot` uses html2canvas which renders a snapshot that **lags behind the actual DOM state by several seconds**. The screenshot will frequently show the PREVIOUS page after a navigation. **Trust the DOM snapshot and JS queries over screenshots for determining page state.** Screenshots are only useful for rough visual inspection, not for verifying which page you're on.

## Critical: Theme resets on locale change

`window.__setLocale(...)` reloads the page, which **resets the theme back to Material** and **resets color scheme to light**. Phases 4 and 5 will always run in Material theme. Use `home-new-message-fab` (Material) not `home-new-message-link` (iOS) in those phases.

## Important: Navigation via UI elements only

**NEVER navigate by setting `window.location.href` or using URL bar navigation.** Always click through the UI using `data-testid` selectors. This tests real navigation paths and catches broken links/buttons.

The full selector registry is in `ui/tests/selectors.ts`. Key selectors are listed inline below using the `[data-testid="..."]` format.

For Konsta `ListInput` components, `data-testid` lands on the outer `<li>`. To type into one, use `[data-testid="..."] input` (or `textarea` for text areas).

---

## Interaction patterns (webview_execute_js)

Since the dedicated MCP interaction tools don't work, use these `webview_execute_js` patterns:

### Clicking an element

```js
document.querySelector('[data-testid="some-button"]').click()
```

### Typing into an input (Svelte-compatible)

Must use native value setter to trigger Svelte reactivity:

```js
(() => {
  const input = document.querySelector('[data-testid="some-input"] input');
  const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
  setter.call(input, 'the value to type');
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
})()
```

For `<textarea>` elements, use `HTMLTextAreaElement.prototype` instead of `HTMLInputElement.prototype`, and select with `textarea` instead of `input`.

### Typing into the message textarea

```js
(() => {
  const ta = document.querySelector('[data-testid="message-input-textarea"]');
  const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set;
  setter.call(ta, 'Hello from Alice!');
  ta.dispatchEvent(new Event('input', { bubbles: true }));
  ta.dispatchEvent(new Event('change', { bubbles: true }));
})()
```

### Waiting for an element to appear

Use polling with a promise:

```js
await new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject('Timeout waiting for element'), 15000);
  const check = () => {
    if (document.querySelector('[data-testid="target-element"]')) {
      clearTimeout(timeout);
      resolve(true);
    } else {
      setTimeout(check, 200);
    }
  };
  check();
})
```

### Click and wait (combined — preferred)

Combine click + wait into a single call to minimize round trips:

```js
(() => {
  document.querySelector('[data-testid="some-button"]').click();
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject('Timeout'), 15000);
    const check = () => {
      if (document.querySelector('[data-testid="next-page-element"]')) {
        clearTimeout(timeout);
        resolve(true);
      } else {
        setTimeout(check, 200);
      }
    };
    setTimeout(check, 100);
  });
})()
```

### Extracting QR code value

The `wa-qr-code` web component exposes `.value` as a JS property, NOT as an HTML attribute. **Do NOT use `getAttribute('value')`** — it returns null.

```js
(() => document.querySelector('wa-qr-code')?.value)()
```

---

## Per-page checks

Run these checks on **every page visit** throughout all phases. **Call all three in parallel** in a single message (three separate tool calls):

1. **Screenshot** — `webview_screenshot` for visual inspection.
2. **Structure snapshot** — `webview_dom_snapshot` (type: `structure`) for structural check.
3. **Overflow detection** — `webview_execute_js`:
   ```js
   (() => {
     const issues = [];
     if (document.documentElement.scrollWidth > document.documentElement.clientWidth)
       issues.push('Page has horizontal overflow');
     document.querySelectorAll('*').forEach(el => {
       if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
         const text = el.textContent?.substring(0, 50);
         if (text?.trim()) issues.push(`Overflow in <${el.tagName.toLowerCase()}>: "${text}"`);
       }
     });
     return issues.slice(0, 20);
   })()
   ```
4. **RTL check** (Farsi phase only) — add this to the overflow detection JS or run as a fourth parallel call:
   ```js
   (() => ({
     dir: document.documentElement.dir,
     direction: getComputedStyle(document.body).direction
   }))()
   ```

Collect all issues found into a running list for the final report.

---

## Phase 0: Start dev environment

1. Invoke the `start-dev` skill to launch both agents, the UI dev server, mailbox server, and stores watcher. **Start Agent 2** as well (it's needed for p2p testing).
2. Wait for Agent 1's task output to contain `MCP Bridge plugin initialized` — extract the actual port.
3. Wait for Agent 2's task output to contain `MCP Bridge plugin initialized` — extract the actual port.
4. Connect to Agent 1 via `driver_session` (start, port from step 2).

---

## Phase 1: Full workflow test (Material + English)

This is the core functional test using both app instances.

### 1.1 Create profile on Agent 1

- Run per-page checks on the CreateProfile screen.
- Wait for `[data-testid="create-profile-name"]` using `webview_execute_js` polling.
- Type "Alice" into `[data-testid="create-profile-name"] input` using the native value setter pattern.
- Type "Test" into `[data-testid="create-profile-surname"] input` using the native value setter pattern.
- Click `[data-testid="create-profile-create-btn"]` (Material) or `[data-testid="create-profile-create-link"]` (iOS) via `webview_execute_js`.
- Wait for `[data-testid="all-chats-list"]` (home page loaded) via `webview_execute_js` polling.
- Run per-page checks.

### 1.2 Create profile on Agent 2

- Connect to Agent 2 via `driver_session` (start, port from Phase 0 step 3).
- Run per-page checks on the CreateProfile screen.
- Type "Bob" into `[data-testid="create-profile-name"] input`.
- Type "Tester" into `[data-testid="create-profile-surname"] input`.
- Click `[data-testid="create-profile-create-btn"]`.
- Wait for `[data-testid="all-chats-list"]`.
- Run per-page checks.

### 1.3 Exchange contact codes

**On Agent 1** (switch appIdentifier to Agent 1's port):

1. Click `[data-testid="home-settings-link"]`, wait for settings page.
2. Click `[data-testid="settings-qr-link"]` (or navigate via profile → `[data-testid="profile-qr-link"]`), wait for add-contact page (`[data-testid="add-contact-back"]`).
3. Run per-page checks.
4. Extract Agent 1's contact code (**use `.value` property, NOT `getAttribute`**):
   ```js
   (() => document.querySelector('wa-qr-code')?.value)()
   ```
5. Save this as `agent1Code`.

**On Agent 2** (switch appIdentifier to Agent 2's port):

1. Navigate to add-contact page via same path (settings → QR link).
2. Run per-page checks.
3. Extract Agent 2's contact code (same `.value` script). Save as `agent2Code`.
4. Type `agent1Code` into `[data-testid="add-contact-code-input"] input` using native value setter.
5. Wait for navigation to direct chat (poll for `[data-testid="direct-chat-messages"]`).

**On Agent 1** (switch back):

1. Type `agent2Code` into `[data-testid="add-contact-code-input"] input`.
2. Wait for navigation to direct chat.

### 1.4 Direct chat

**On Agent 1:**

- Verify the direct chat with Bob is open. Run per-page checks.
- Type "Hello from Alice!" into `[data-testid="message-input-textarea"]` using the **textarea** native value setter pattern.
- Click `[data-testid="message-input-send"]`.
- Verify the message appears in `[data-testid="direct-chat-messages"]`.

**On Agent 2** (switch appIdentifier):

- The direct chat with Alice should already be open (from contact exchange).
- Wait for "Hello from Alice!" to appear via polling:
  ```js
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject('Timeout'), 15000);
    const check = () => {
      if (document.querySelector('[data-testid="direct-chat-messages"]')?.textContent?.includes('Hello from Alice!')) {
        clearTimeout(timeout);
        resolve(true);
      } else {
        setTimeout(check, 500);
      }
    };
    check();
  })
  ```
- Run per-page checks.
- Type "Hello from Bob!" into `[data-testid="message-input-textarea"]`.
- Click `[data-testid="message-input-send"]`.

**On Agent 1** (switch back):

- Verify "Hello from Bob!" appears (same polling pattern).

### 1.5 Chat settings

**On Agent 1:**

- Click `[data-testid="direct-chat-settings-link"]`, wait for `[data-testid="chat-settings-back"]`.
- Run per-page checks.
- Verify peer name via `webview_execute_js`:
  ```js
  (() => document.querySelector('[data-testid="chat-settings-peer-name"]')?.textContent)()
  ```
- Click `[data-testid="chat-settings-search-btn"]`, wait for search view.
- Run per-page checks on the search view.
- Click `[data-testid="direct-chat-search-back"]` to exit search, then `[data-testid="direct-chat-back"]` to go home.

### 1.6 All settings pages

**From the home page on Agent 1:**

Navigate through each page, running per-page checks. Combine click+wait into single JS calls to move fast:

1. Click `[data-testid="home-settings-link"]` → wait for settings page. Per-page checks.
2. Click `[data-testid="settings-profile-link"]` → wait for profile page. Per-page checks.
3. Click `[data-testid="profile-edit-name"]` → per-page checks → click `[data-testid="edit-name-back"]`.
4. Click `[data-testid="profile-edit-about"]` → per-page checks → click `[data-testid="edit-about-back"]`.
5. Click `[data-testid="profile-edit-photo"]` → per-page checks → click `[data-testid="edit-photo-back"]`.
6. Click `[data-testid="profile-back"]` → back to settings.
7. Click `[data-testid="settings-appearance-link"]` → per-page checks → click `[data-testid="appearance-back"]`.
8. Click `[data-testid="settings-account-link"]` → per-page checks → click `[data-testid="account-back"]`.
9. Click `[data-testid="settings-back"]` → return home.

### 1.7 New message

**From the home page:**

1. Click `[data-testid="home-new-message-fab"]` (Material) → per-page checks → click `[data-testid="new-message-back"]`.

### 1.8 New group UI

**From the home page:**

1. Click `[data-testid="home-new-message-fab"]`, then find and click the new-group link.
2. Wait for `[data-testid="new-group-back"]`. Per-page checks.
3. Click `[data-testid="new-group-next-btn"]` (Material) or `[data-testid="new-group-next-link"]` (iOS).
4. Wait for `[data-testid="new-group-info-back"]`. Per-page checks.
5. Click `[data-testid="new-group-info-back"]` → click `[data-testid="new-group-back"]` → click `[data-testid="new-message-back"]` to return home.

**Note**: Group chat backend commands are commented out — only test navigation/UI, not actual group creation.

### 1.9 Home page final state

- Verify you're on `/` by checking for `[data-testid="all-chats-list"]`.
- Run per-page checks on the chats list.

---

## Layout modes: Desktop and Mobile

The app has two layouts controlled by the `isWideScreen` store (`ui/src/lib/stores/screen.svelte.ts`):

- **Desktop**: Two-panel layout (sidebar 320px + content area) via `DesktopLayout.svelte`.
- **Mobile**: Single-panel, full-width pages with back navigation.

The Tauri dev window is wide enough to trigger desktop layout by default. To switch layouts:

```js
// Switch to mobile layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));

// Switch back to desktop layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));
```

The override persists across SvelteKit client-side navigations but is **lost on full page reloads** (e.g., after `__setLocale()`). Re-dispatch the event after any reload.

**Key layout differences:**
- Back buttons: hidden on desktop, visible on mobile
- FAB: shown on mobile Material, hidden on desktop
- GetStarted card: fixed on mobile, in content area on desktop
- New message: navbar link (iOS mobile), FAB (Material mobile), sidebar icon (desktop)

---

## Combination matrix

All visual passes must cover **every combination** of theme, language, layout, and color scheme (16 total):

| # | Theme | Language | Layout | Color | Phase |
|---|-------|----------|--------|-------|-------|
| 1 | Material | English | Desktop | Light | 1 (functional test) |
| 2 | Material | English | Mobile | Light | 2 |
| 3 | iOS | English | Desktop | Light | 2 |
| 4 | iOS | English | Mobile | Light | 2 |
| 5 | Material | English | Desktop | Dark | 3 |
| 6 | Material | English | Mobile | Dark | 3 |
| 7 | iOS | English | Desktop | Dark | 3 |
| 8 | iOS | English | Mobile | Dark | 3 |
| 9 | Material | German | Desktop | Light | 4 |
| 10 | Material | German | Mobile | Light | 4 |
| 11 | iOS | German | Desktop | Light | 4 |
| 12 | iOS | German | Mobile | Light | 4 |
| 13 | Material | Farsi | Desktop | Light | 5 |
| 14 | Material | Farsi | Mobile | Light | 5 |
| 15 | iOS | Farsi | Desktop | Light | 5 |
| 16 | iOS | Farsi | Mobile | Light | 5 |

### Switching between combinations

Within a language (no reload needed):

```js
// Switch theme
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'ios' } }));  // or 'material'

// Switch layout
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));  // mobile
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));   // desktop
```

When changing language (`__setLocale()` triggers a full reload):
1. Theme resets to Material — re-apply iOS if needed.
2. Layout resets to desktop (media query re-evaluates) — re-dispatch `set-wide-screen` if needed.
3. For Farsi, also set `document.documentElement.dir = 'rtl'` after reload.

---

## Page visit list

For each combination, navigate to every page using `data-testid` click paths and run per-page checks. Use theme-appropriate selectors:

- **Material**: `[data-testid="home-new-message-fab"]`, `[data-testid="create-profile-create-btn"]`, `[data-testid="new-group-next-btn"]`
- **iOS**: `[data-testid="home-new-message-link"]`, `[data-testid="create-profile-create-link"]`, `[data-testid="new-group-next-link"]`

Pages to visit:

1. `/` (home) — `[data-testid="all-chats-list"]`
2. `/new-message` — click new-message button (theme-dependent), back via `[data-testid="new-message-back"]`
3. `/new-message/add-contact` — navigate through new-message, back via `[data-testid="add-contact-back"]`
4. `/new-group` — navigate through new-message
5. `/direct-chats/{agentId}` — click chat item in list, back via `[data-testid="direct-chat-back"]`
6. `/direct-chats/{agentId}/chat-settings` — click `[data-testid="direct-chat-settings-link"]`, back via `[data-testid="chat-settings-back"]`
7. `/settings` — click `[data-testid="home-settings-link"]`
8. `/settings/profile` — click `[data-testid="settings-profile-link"]`
9. `/settings/profile/edit-name` — click `[data-testid="profile-edit-name"]`, back via `[data-testid="edit-name-back"]`
10. `/settings/profile/edit-about` — click `[data-testid="profile-edit-about"]`, back via `[data-testid="edit-about-back"]`
11. `/settings/profile/edit-photo` — click `[data-testid="profile-edit-photo"]`, back via `[data-testid="edit-photo-back"]`
12. `/settings/profile/add-contact` — click `[data-testid="profile-add-contact"]`, back via `[data-testid="add-contact-back"]`
13. `/settings/appearance` — back to settings, click appearance link, back
14. `/settings/account` — click `[data-testid="settings-account-link"]`, back via `[data-testid="account-back"]`
15. Return home via `[data-testid="settings-back"]`

---

## Phase 2: English visual pass (remaining 3 combinations)

Phase 1 already covered Material + English + Desktop. Now cover:

### 2a: Material + English + Mobile
```js
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));
```
Visit every page. Focus on back buttons, FAB visibility, GetStarted card position.

### 2b: iOS + English + Desktop
```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'ios' } }));
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));
```
Focus on iOS-specific navbar style, save/next links vs FABs, list inset styling.

### 2c: iOS + English + Mobile
```js
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));
```

---

## Phase 3: Dark mode pass (English, 4 combinations)

Switch to dark mode (no page reload needed):
```js
document.documentElement.classList.add('dark', 'wa-dark');
```

Or click through the UI: navigate to appearance settings and click `[data-testid="appearance-dark"]`.

To switch back to light mode:
```js
document.documentElement.classList.remove('dark', 'wa-dark');
```

For each of the 4 combinations (Material Desktop, Material Mobile, iOS Desktop, iOS Mobile), enable dark mode and visit every page. Focus on:

- **Background colors** — pages should have dark backgrounds, not white/light
- **Text contrast** — text should be light on dark backgrounds, fully readable
- **Input fields** — textarea and input backgrounds should be dark, not white
- **Message bubbles** — sent/received bubbles should have appropriate dark-mode colors
- **Navbar/toolbar** — should use dark surface colors
- **List items** — backgrounds should be dark, separators visible
- **Icons and buttons** — should be visible against dark backgrounds
- **No hardcoded light colors** — look for elements that stay white/light in dark mode (e.g. `bg-white` without `dark:bg-*` counterpart)

### Dark mode detection check

Run this alongside the overflow check on every page:
```js
(() => {
  const isDark = document.documentElement.classList.contains('dark');
  const issues = [];
  if (!isDark) issues.push('Dark mode class not active');
  // Check for elements with hardcoded white backgrounds
  document.querySelectorAll('*').forEach(el => {
    const bg = getComputedStyle(el).backgroundColor;
    // Flag pure white backgrounds in dark mode (excluding transparent/hidden elements)
    if (bg === 'rgb(255, 255, 255)' && el.offsetWidth > 0 && el.offsetHeight > 0) {
      const tag = el.tagName.toLowerCase();
      const id = el.getAttribute('data-testid') || el.className?.toString().substring(0, 40) || '';
      // Skip web component internals and known safe elements
      if (!el.closest('wa-icon, wa-avatar, wa-qr-code'))
        issues.push(`White bg in dark mode: <${tag}> ${id}`);
    }
  });
  return { isDark, issues: issues.slice(0, 20) };
})()
```

### 3a: Material + English + Desktop + Dark
Already in Material Desktop from Phase 2. Just enable dark mode.

### 3b: Material + English + Mobile + Dark
```js
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));
```

### 3c: iOS + English + Desktop + Dark
```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'ios' } }));
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: true }));
```

### 3d: iOS + English + Mobile + Dark
```js
window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: false }));
```

After completing all 4 dark-mode combinations, switch back to light mode:
```js
document.documentElement.classList.remove('dark', 'wa-dark');
```

---

## Phase 4: German (de-de) pass (4 combinations)

Switch locale (triggers full page reload):
```js
window.__setLocale('de-de');
```

Wait for reload, reconnect `driver_session` if needed. For each of the 4 combinations (Material Desktop, Material Mobile, iOS Desktop, iOS Mobile), re-apply theme/layout overrides and visit every page. Focus on:

- Text overflow in buttons, navbars, and list items (German words are longer)
- Truncation issues and layout breakage
- Verify navbar text is translated (not English)

### Recommended batching pattern

```js
(() => {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject('Timeout'), 15000);
    const results = [];
    const waitFor = (sel, cb) => {
      const c = () => { if (document.querySelector(sel)) cb(); else setTimeout(c, 200); };
      setTimeout(c, 200);
    };
    const clickAndWait = (clickSel, waitSel, cb) => {
      const el = document.querySelector(clickSel + ' a') || document.querySelector(clickSel);
      if (el) el.click();
      waitFor(waitSel, cb);
    };

    clickAndWait('[data-testid="home-settings-link"]', '[data-testid="settings-profile-link"]', () => {
      results.push({ page: 'settings', navbar: document.querySelector('.k-navbar')?.textContent?.trim() });
      clearTimeout(timeout);
      resolve(results);
    });
  });
})()
```

---

## Phase 5: Farsi (fa-ir) RTL pass (4 combinations)

Switch locale:
```js
window.__setLocale('fa-ir');
```

Wait for reload, reconnect `driver_session` if needed. Then set RTL:
```js
document.documentElement.dir = 'rtl';
```

For each of the 4 combinations, re-apply `dir = 'rtl'` and theme/layout overrides, then visit every page. Focus on:

- RTL text direction (`dir="rtl"` on `<html>`)
- Mirrored navigation (back buttons on right)
- Correct alignment of message bubbles
- Navbar layout and icon/text alignment
- Verify navbar text is translated to Farsi

---

## Phase 6: Reset & cleanup

Reset to English + Material on Agent 1:

```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'material' } }));
```

```js
window.__setLocale('en');
```

Stop all driver sessions via `driver_session` (action: `stop`).

Kill all background dev processes (Tauri agents, mailbox server, stores watcher, UI dev server) via `TaskStop` on each saved task ID from Phase 0.

---

## Phase 7: Report

Compile a summary of **all issues found**, categorized by:

- **Layout/overflow** — elements overflowing their containers, text truncation, horizontal scroll
- **Theme-specific** — issues only present in iOS or Material theme
- **Dark mode** — elements with wrong background/text colors in dark mode, hardcoded light colors, poor contrast
- **Locale-specific** — missing translations, German text overflow, RTL alignment problems
- **Functional** — broken interactions, navigation failures, missing data

For each issue, include:
- The page/route where it was found
- The phase/theme/locale
- A description of the problem
- Whether it's a blocker or cosmetic

If no issues were found, report a clean bill of health.
