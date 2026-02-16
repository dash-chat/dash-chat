---
name: review-app
description: "Run a full app review — launches two instances, walks through every workflow, and checks all screens with iOS/Material themes and Farsi/German translations."
user-invocable: true
allowed-tools: mcp__tauri__driver_session, mcp__tauri__webview_screenshot, mcp__tauri__webview_dom_snapshot, mcp__tauri__webview_find_element, mcp__tauri__webview_execute_js, mcp__tauri__webview_wait_for, mcp__tauri__webview_interact, mcp__tauri__webview_keyboard, mcp__tauri__webview_get_styles, mcp__tauri__read_logs, mcp__tauri__manage_window, mcp__tauri__ipc_execute_command, mcp__tauri__ipc_monitor, mcp__tauri__ipc_get_captured
---

# Review App

A structured runbook for smoke-testing Dash Chat end-to-end. Launches two app instances, exercises the full user workflow, and visually inspects every screen across Konsta UI themes (iOS + Material) and locales (English, German, Farsi).

## Important: Navigation via UI elements only

**NEVER navigate by setting `window.location.href` or using URL bar navigation.** Always click through the UI using `data-testid` selectors. This tests real navigation paths and catches broken links/buttons.

The full selector registry is in `ui/tests/selectors.ts`. Key selectors are listed inline below using the `[data-testid="..."]` format.

For Konsta `ListInput` components, `data-testid` lands on the outer `<li>`. To type into one, use `[data-testid="..."] input` (or `textarea` for text areas).

---

## Per-page checks

Run these checks on **every page visit** throughout all phases:

1. **Screenshot** — `webview_screenshot` for visual inspection.
2. **Accessibility snapshot** — `webview_dom_snapshot` (type: `accessibility`) for structural check.
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
4. **RTL check** (Farsi phase only) — additionally verify:
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
- Wait for `[data-testid="create-profile-name"]`.
- Type "Alice" into `[data-testid="create-profile-name"] input`.
- Type "Test" into `[data-testid="create-profile-surname"] input`.
- Click `[data-testid="create-profile-create-btn"]` (Material) or `[data-testid="create-profile-create-link"]` (iOS).
- Wait for `[data-testid="all-chats-list"]` (home page loaded).
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

1. Click `[data-testid="home-contacts-link"]` to go to contacts.
2. Click `[data-testid="contacts-add-link"]` to go to add-contact.
3. Run per-page checks.
4. Extract Agent 1's contact code:
   ```js
   (() => document.querySelector('wa-qr-code')?.getAttribute('value'))()
   ```
5. Save this as `agent1Code`.

**On Agent 2** (switch appIdentifier to Agent 2's port):

1. Click `[data-testid="home-contacts-link"]`.
2. Click `[data-testid="contacts-add-link"]`.
3. Run per-page checks.
4. Extract Agent 2's contact code (same script as above). Save as `agent2Code`.
5. Type `agent1Code` into `[data-testid="add-contact-code-input"] input`.
6. Wait for navigation to direct chat (wait for `[data-testid="direct-chat-messages"]`).

**On Agent 1** (switch back):

1. Type `agent2Code` into `[data-testid="add-contact-code-input"] input`.
2. Wait for navigation to direct chat.

### 1.4 Direct chat

**On Agent 1:**

- Verify the direct chat with Bob is open. Run per-page checks.
- Type "Hello from Alice!" into `[data-testid="message-input-textarea"]`.
- Click `[data-testid="message-input-send"]`.
- Verify the message appears in `[data-testid="direct-chat-messages"]`.

**On Agent 2** (switch appIdentifier):

- The direct chat with Alice should already be open (from contact exchange).
- Wait for "Hello from Alice!" to appear.
- Run per-page checks.
- Type "Hello from Bob!" into `[data-testid="message-input-textarea"]`.
- Click `[data-testid="message-input-send"]`.

**On Agent 1** (switch back):

- Verify "Hello from Bob!" appears.

### 1.5 Chat settings

**On Agent 1:**

- Click `[data-testid="direct-chat-settings-link"]` (the navbar title area).
- Wait for `[data-testid="chat-settings-back"]`.
- Run per-page checks.
- Verify peer name via `[data-testid="chat-settings-peer-name"]`.
- Click `[data-testid="chat-settings-search-btn"]` to test search navigation.
- Run per-page checks on the search view.
- Click `[data-testid="direct-chat-back"]` to go back to home.

### 1.6 All settings pages

**From the home page on Agent 1:**

1. Click `[data-testid="home-settings-link"]` to go to `/settings`.
   - Run per-page checks.

2. Click `[data-testid="settings-profile-link"]` to go to `/settings/profile`.
   - Run per-page checks.

3. Click `[data-testid="profile-edit-name"]` to go to `/settings/profile/edit-name`.
   - Run per-page checks.
   - Click `[data-testid="edit-name-back"]` to go back.

4. Click `[data-testid="profile-edit-about"]` to go to `/settings/profile/edit-about`.
   - Run per-page checks.
   - Click `[data-testid="edit-about-back"]` to go back.

5. Click `[data-testid="profile-edit-photo"]` to go to `/settings/profile/edit-photo`.
   - Run per-page checks.
   - Navigate to `/settings/profile/edit-photo/text` if a link exists. Run per-page checks.
   - Navigate back to profile.

6. Click `[data-testid="profile-back"]` to go back to `/settings`.

7. Click `[data-testid="settings-account-link"]` to go to `/settings/account`.
   - Run per-page checks.
   - Click `[data-testid="account-back"]` to go back.

8. Click `[data-testid="settings-back"]` to return home.

### 1.7 Contacts & new message

**From the home page:**

1. Click `[data-testid="home-contacts-link"]` to go to `/contacts`.
   - Run per-page checks.
   - Click `[data-testid="contacts-back"]` to go back.

2. Click `[data-testid="home-new-message-fab"]` (Material) to go to `/new-message`.
   - Run per-page checks.
   - Click `[data-testid="new-message-back"]` to go back.

### 1.8 New group UI

**From the home page:**

1. Navigate to `/new-message`, then click the new group link if visible.
   - Or navigate directly: from home click `[data-testid="home-new-message-fab"]`, then look for a new-group link.
2. On the member selection step, wait for `[data-testid="new-group-back"]`. Run per-page checks.
3. Click `[data-testid="new-group-next-btn"]` (Material) or `[data-testid="new-group-next-link"]` (iOS) to advance to group info.
4. Wait for `[data-testid="new-group-info-back"]`. Run per-page checks.
5. Click `[data-testid="new-group-info-back"]` to go back to step 1.
6. Click `[data-testid="new-group-back"]` to go back.
7. Click `[data-testid="new-message-back"]` to return home.

**Note**: Group chat backend commands are commented out — only test navigation/UI, not actual group creation.

### 1.9 Home page final state

- Verify you're on `/` by checking for `[data-testid="all-chats-list"]`.
- Run per-page checks on the chats list.

---

## Phase 2: iOS theme visual pass

Switch to iOS theme without reload (on Agent 1):

```js
window.dispatchEvent(new CustomEvent('theme-change', { detail: { theme: 'ios' } }));
```

Re-visit **every page** from Phase 1 (except the profile creation and contact exchange — just navigate to each screen using `data-testid` selectors), running per-page checks at each. Focus on iOS-specific differences:

- Navbar style (large title, back button style)
- Save/next links in top-right (e.g., `[data-testid="create-profile-create-link"]`, `[data-testid="edit-name-save-link"]`) vs Material FABs/buttons
- List inset styling
- Tabbar vs buttons on add-contact
- `[data-testid="home-new-message-link"]` (iOS) vs `[data-testid="home-new-message-fab"]` (Material)

Navigate to each page using the same click paths as Phase 1:

1. `/` (home) — check `[data-testid="all-chats-list"]`
2. `/contacts` — click `[data-testid="home-contacts-link"]`, then back via `[data-testid="contacts-back"]`
3. `/new-message` — click `[data-testid="home-new-message-link"]` (iOS), then back via `[data-testid="new-message-back"]`
4. `/new-group` — navigate through new-message
5. `/add-contact` — click `[data-testid="home-contacts-link"]` then `[data-testid="contacts-add-link"]`, then back via `[data-testid="add-contact-back"]`
6. `/direct-chats/{agentId}` — click the chat item in the list, then back via `[data-testid="direct-chat-back"]`
7. `/direct-chats/{agentId}/chat-settings` — click `[data-testid="direct-chat-settings-link"]`, then back via `[data-testid="chat-settings-back"]`
8. `/settings` — click `[data-testid="home-settings-link"]`
9. `/settings/profile` — click `[data-testid="settings-profile-link"]`
10. `/settings/profile/edit-name` — click `[data-testid="profile-edit-name"]`, then back via `[data-testid="edit-name-back"]`
11. `/settings/profile/edit-about` — click `[data-testid="profile-edit-about"]`, then back via `[data-testid="edit-about-back"]`
12. `/settings/profile/edit-photo` — click `[data-testid="profile-edit-photo"]`, then back
13. `/settings/account` — back to settings, click `[data-testid="settings-account-link"]`, then back via `[data-testid="account-back"]`
14. Return home via `[data-testid="settings-back"]`

---

## Phase 3: German (de-de) translation pass

Switch locale on Agent 1 via `webview_execute_js`:

```js
window.__setLocale('de-de');
```

This sets the cookie + global variable and reloads the page. Wait for the page to reload, then reconnect `driver_session` if needed.

Navigate to **every page** listed in Phase 2 using the same `data-testid` click paths, running per-page checks at each. Focus on:

- Text overflow in buttons, navbars, and list items (German words are significantly longer than English)
- Truncation issues
- Layout breakage from long words

---

## Phase 4: Farsi (fa-ir) RTL pass

Switch locale on Agent 1:

```js
window.__setLocale('fa-ir');
```

Wait for reload, reconnect `driver_session` if needed.

Navigate to **every page** listed in Phase 2 using the same `data-testid` click paths, running per-page checks at each (including the RTL-specific checks). Focus on:

- RTL text direction (`dir="rtl"` on `<html>`)
- Mirrored navigation (back buttons on right, etc.)
- Correct alignment of message bubbles
- Navbar layout
- Icon/text alignment

---

## Phase 5: Reset & cleanup

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

## Phase 6: Report

Compile a summary of **all issues found**, categorized by:

- **Layout/overflow** — elements overflowing their containers, text truncation, horizontal scroll
- **Theme-specific** — issues only present in iOS or Material theme
- **Locale-specific** — missing translations, German text overflow, RTL alignment problems
- **Functional** — broken interactions, navigation failures, missing data

For each issue, include:
- The page/route where it was found
- The phase/theme/locale
- A description of the problem
- Whether it's a blocker or cosmetic

If no issues were found, report a clean bill of health.
