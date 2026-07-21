# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Dash Chat is an end-to-end encrypted messenger built with Svelte 5 (frontend) and Rust/Tauri (backend), using p2panda for peer-to-peer communication. The application works both with and without internet connectivity.

**Current Status**: Pre-alpha, being rebuilt on top of p2panda.

## Signal UX Reference

Dash Chat aims to match Signal's UX as closely as possible. A private repository of Signal screenshots (Android + iOS) is available at `dash-chat/signal-screenshots`.

**Setup (run once per session if needed):**
```bash
# Clone if not already present (gitignored)
[ -d signal-reference ] || gh repo clone dash-chat/signal-screenshots signal-reference
```

**When building or modifying UI, you MUST:**
1. Read `signal-reference/manifest.json` to find the relevant Signal screenshots for the Dash Chat route you're working on.
2. Read the corresponding screenshots (both `android/` and `ios/` when available) to understand Signal's layout, spacing, typography, colors, and interaction patterns.
3. Model your implementation after Signal's UX. Match the overall feel, not pixel-perfect details — adapt for Konsta UI components and our existing patterns.
4. When verifying your UI changes, compare your screenshots against the Signal reference.

**Directory structure:**
```
signal-reference/
├── manifest.json          # Maps Signal sections → Dash Chat routes
├── android/               # Android (Material) screenshots
│   ├── home/              # Chat list, search, overflow menu
│   ├── create-account/    # Onboarding flow
│   ├── direct-chat/       # 1:1 chat view + chat-settings/
│   ├── group-chat/        # Group chat view
│   ├── message-types/     # Image/voice/reactions/context menu
│   ├── new-message/       # Contact picker + new-group/
│   └── settings/          # All settings sub-pages
└── ios/                   # iOS screenshots (same structure)
└── desktop/                   # Desktop screenshots (same structure)
```

Screenshots are named descriptively with sequence prefixes (e.g., `01-chat-list-empty.png`, `02-overflow-menu-open.png`). Browse the directory listing to find what you need.

## General Coding Style

Please read this coding style carefully and take it into account when planning or coding:

- Never make assumptions about undocumented APIs or configurations.
- Ask clarifying questions if a task's requirements are ambiguous.
- Modify only the minimum necessary lines of code to achieve the goal.
- Avoid refactoring adjacent or unrelated files unless explicitly asked.
- Match existing style, even if you would write it differently.
- Do not write speculative helper functions or complex abstractions.
- Prioritize simple, readable code over clever or DRY patterns.
- Establish clear test or verification criteria before writing any code.
- Run local tests or build steps to verify your changes actually work before completion.
- When a function/component gets too long or mixes high-level and low-level code, extract the lower-level part into a well-named helper that's a meaningful reusable unit — then use it at every site that needs it, not just the one you split it from.
- Each function/component should do one thing at a single level of abstraction: the caller says *what*, the helper handles *how*.
- A component must render correctly wherever it's placed and never alter its parent's UI — no outer margin on its root (margins leak outward); use padding for internal spacing instead.
- Spacing *between* children belongs to the parent (use `gap`); the child owns its internal layout, the parent owns where the child sits.
- Try to remain as simple as possible with your implementations.
- Try to reuse types and functions across the project rather than reimplement them.
- Don't use `any` or `unknown` typescript types. Instead, try to understand the actual typescript types and use them to infer the appropriate data structures and algorithms to use.
- **Don't implicitly cast numbers to booleans.** In conditions (`if`, `{#if}`, `&&`, ternaries) compare numbers explicitly — write `arr.length > 0`, not `arr.length`; `count !== 0`, not `count`. Truthiness on a number silently treats `0` as false, which hides intent and is a common source of off-by-one/empty-state bugs. Use `!!value` only to coerce a genuinely non-numeric value (object/string) to a boolean.
- Prefer Tailwind CSS utility classes over custom CSS styles whenever possible. Use inline `class` attributes with Tailwind classes instead of adding styles to `<style>` blocks.
- **Write very few comments.** Default to none. The only two acceptable reasons to add a comment are:
  1. Documenting what a function does (a doc-comment on the function signature). Skip these for self-explanatory helpers whose name and signature already say everything.
  2. Explaining *why* a non-obvious piece of code is there — a hidden constraint, a workaround for a specific bug, a subtle invariant that would surprise a reader. Keep these to one or two lines, no more.
  Anti-patterns — do NOT write these:
  - **What-comments**: restating what the code does when well-named identifiers already say it.
  - **Derivation comments**: walking the reader through the reasoning that produced the formula or condition right below ("In LTR X is on the right; in RTL it's on the left; therefore we flip Y..."). The formula is the artifact. The derivation belongs in the commit message or your head, not in the source. A reader having to think for a few seconds to recompute the reasoning is the normal cost of reading code, not a signal to comment.
  - **Narrative-of-change comments**: notes about the edit you just made ("added X to support Y", "renamed for clarity", "moved from foo.ts"). That's PR-description territory.
  - **Stale TODOs**: things that belong in the issue tracker.
  Self-test before keeping a comment: "If I delete this, would a reader of the surrounding code be genuinely confused about *why* something is the way it is, or just have to read the code?" If only the latter, delete it. When in doubt, delete it.
- **Avoid deep nesting.** If a function is going to exceed two levels of nesting (e.g. an `if` inside a `for` inside another block, or three nested callbacks), extract the inner work into clearly named utility functions that each do one specific task and call them from the higher-level function. Flat code with named helpers is easier to read, test, and re-arrange than a single deeply nested function.
- **STRICT: Use logical `start`/`end` CSS properties, not directional `left`/`right`.** The app is fully RTL-aware (Farsi is a supported locale) and directional left/right properties break in RTL.
  - Tailwind: use `ms-`, `me-`, `ps-`, `pe-`, `start-`, `end-`, `rounded-s-`, `rounded-e-`, `rounded-ss-`, `rounded-se-`, `rounded-es-`, `rounded-ee-`, `text-start`, `text-end`, `border-s`, `border-e`. Do NOT use `ml-`, `mr-`, `pl-`, `pr-`, `left-`, `right-`, `rounded-l-`, `rounded-r-`, `rounded-tl-`/`tr-`/`bl-`/`br-`, `text-left`, `text-right`, `border-l`, `border-r`.
  - Raw CSS: use `margin-inline-start/end`, `padding-inline-start/end`, `inset-inline-start/end`, `border-inline-start/end`, `border-start-start-radius`/`border-start-end-radius`/`border-end-start-radius`/`border-end-end-radius`, `text-align: start/end`. Do NOT use `margin-left/right`, `padding-left/right`, `left:`/`right:`, `border-left/right`, `border-*-left/right-radius`, `text-align: left/right`.
  - The ONLY acceptable uses of `left`/`right` are cases where RTL genuinely does not apply: viewport-pixel coordinates computed from `getBoundingClientRect()`, symmetric positioning where both sides are set to the same value (prefer `inset-inline: X` or `inset-x-X` instead), or rotation transforms. In every other case use logical properties.
  - **When reviewing code, flag any new `left`/`right` CSS property or directional Tailwind class as a defect and ask the author to convert it to the logical equivalent (or justify why RTL doesn't apply).**

## Development Environment

### Prerequisites
- Rust (https://rust-lang.org/tools/install/)
- pnpm (version >=9.0.0)
- Tauri prerequisites for your platform (https://tauri.app/start/prerequisites/)
- Alternatively: Use `nix develop` for a Nix development shell

### Initial Setup
```bash
pnpm install
```

## Common Commands

### Running the Application
```bash
# Start two instances forming a p2panda network
pnpm start

# This uses mprocs to spawn multiple processes:
# - agent1 and agent2: Two Tauri development instances
# - ui: Frontend development server
# - stores: Watches and rebuilds the stores package
```

### Development Tasks
```bash
# Run Rust tests
cargo nextest run
# or
pnpm test

# Type check Svelte components (from ui/ directory)
pnpm check
pnpm check:watch

# Build UI (from ui/ directory)
pnpm build

# Build stores package (from packages/stores/ directory)
pnpm build
```

### Mobile Development
```bash
# Run on Android
pnpm tauri android dev

# View Android logs
adb logcat | grep -F "`adb shell ps | grep studio.darksoil.dashchat | tr -s [:space:] ' ' | cut -d' ' -f2`"

# Run on iOS simulator
pnpm tauri ios dev "iPhone 16"

# Run on physical iOS device
pnpm tauri ios dev --device
```

## Architecture

### Monorepo Structure

This is a pnpm workspace with multiple packages:
- **ui/**: Svelte 5 + TypeScript frontend (SvelteKit application)
- **packages/stores/**: Shared TypeScript stores for state management
- **packages/site/**: Marketing/download site
- **e2e-tests/**: WebdriverIO E2E test suite
- **crates/dashchat-node/**: Core p2p backend logic (Rust)
- **crates/mailbox-server/**: HTTP server for offline message storage
- **src-tauri/**: Tauri application wrapper and integration layer

### Backend Architecture (Rust)

The Rust workspace is one Cargo workspace covering the Tauri app crate (`src-tauri`) and several library/binary crates under `crates/`.

**dashchat-node** is the p2p core. It owns the `Node`, which manages p2panda topics and spaces — the distributed structures that organize conversations, device groups, contact lists, and profile announcements — and runs p2panda's discovery, networking, sync, encryption, and group-auth layers on top of a local SQLite operation store. Each topic is an append-only log; CRDT semantics resolve concurrent writes. The Node exposes a subscription/notification interface that the host layer consumes for real-time UI updates, and integrates the mailbox client so messages keep flowing when peers are offline.

**src-tauri** is the host for the Svelte UI on desktop and mobile and the glue between the frontend and the Node. It registers a domain-organized library of Tauri commands (profile, contacts, devices, direct/group chats, settings, logs, push notifications) that the frontend invokes, and bridges Node notifications back to the webview as Tauri events. It also wires up the platform-specific story: push notifications, barcode scanner, system bars, virtual-keyboard padding, desktop menus, i18n, log redaction for error reports, and lifecycle management. On desktop it can spawn an in-process mailbox-server advertised via mDNS so peers on the same LAN can sync without any cloud service.

**mailbox-server** is the stateful HTTP relay that holds serialized p2panda operations (blobs) on behalf of offline peers until they reconnect. It supports bidirectional sync — clients pull blobs they are missing and push blobs the server is missing in the same exchange — runs a background cleanup task that drops blobs older than 7 days, and optionally forwards arrival notifications to a push-notifications-server so devices wake up to fetch new traffic. Deployable either as a standalone cloud service or spawned in-process by the Tauri app for LAN-only operation.

**mailbox-client** is the async client the Node uses to talk to a mailbox-server (cloud or local), with a single API regardless of how the server is reached.

**push-notifications-server** registers FCM device tokens per topic and dispatches notifications when the mailbox-server tells it new blobs have arrived for a topic a device is subscribed to. **push-notifications-client** is the small HTTP client both the mailbox-server (for forwarding arrivals) and the Tauri app (for registering tokens) use to reach it.

**dashchat-compat** provides serialization helpers that version on-the-wire and on-disk data structures so old clients keep reading data written by newer ones where the schema is compatible — see also `e2e-tests/compat/`.

**dashchat-utils** is a small cross-crate grab bag (retry helpers, singleton-task management, etc.).

**Key backend patterns:**
- Node held as Tauri-managed state, accessed via `app.state::<Node>()`.
- Async notification channel from Node → frontend over Tauri events.
- All Tauri commands are async and return `Result<T, String>` (stringified errors at the JS boundary).
- Wire format and on-disk operations are CBOR-encoded.

### Frontend Architecture (Svelte 5 + TypeScript)

**Structure:**
- **ui/src/routes/**: SvelteKit file-based routing (see [UI Navigation Map](#ui-navigation-map) below)
- **ui/src/components/**: Reusable UI components
- **ui/src/utils/**: Utility functions (image compression, time formatting, QR codes, etc.)
- **ui/tests/**: Test selectors and page objects (see [UI Test Utilities](#ui-test-utilities) below)
- **packages/stores/src/**: Shared state management
  - Organized by domain: contacts, chats, group-chats, direct-chats, devices
  - Each domain has a `-store.ts` (state) and `-client.ts` (Tauri commands)
  - `p2panda/`: Core p2panda integration (logs-store, logs-client, types)

**Frontend Patterns:**
- Signalium for reactive state management
- **STRICT: All Tauri commands must be invoked via `invokeAfterSetup()` from `packages/stores/src/utils/invoke-after-setup.ts`, never `invoke()` from `@tauri-apps/api` directly.** At startup the webview can invoke node-backed commands before `async_setup` finishes managing the node; `invokeAfterSetup()` retries the transient "state not managed" error until the backend is ready. Importing `invoke` directly from `@tauri-apps/api/core` (outside `invoke-after-setup.ts` itself) is a defect — flag it in review.
- UI built with Konsta UI components (mobile-first design)
- Internationalization using @inlang/paraglide-js
- Image compression before upload
- **iOS theme action buttons**: On actual iOS devices, primary action buttons (Save, Done, Create, Add, Next) appear as a `<Link>` in the Navbar's `right` snippet. On all other platforms (including macOS desktop), they appear as a bottom FAB (`class="fixed-action-btn"`). Use `import { isIos } from '$lib/utils/environment'` and `{#if isIos}` in the navbar right snippet and `{#if !isIos}` around the FAB. Apply disabled styling via `rightClass="ios-right-disabled"` on the Navbar (defined in `app.css`).

### Desktop Layout

On wide screens (≥768px), the app uses a two-panel layout managed by `DesktopLayout.svelte`:
- **Sidebar** (left, 280px): Shows the contextual panel based on the current route — `ChatListPanel` for chat routes, `SettingsPanel` for `/settings/*`, `NewMessagePanel` for `/new-message/*`.
- **Content** (right, flex): Shows the page content. For sidebar-only routes (`/` and `/settings`), an `EmptyState` placeholder is rendered instead.

Pages like `/`, `/settings`, and `/new-message` always render their mobile content (wrapped in `<Page>`). On desktop, `DesktopLayout` handles showing the correct sidebar panel and decides whether to render `EmptyState` or the page's children in the content area. Pages never check `isWideScreen` to decide between EmptyState and their content — that logic lives solely in `DesktopLayout`.

**Sidebar panel switching without navigation (`pushState`):** On desktop, clicking "new message" from the `ChatListPanel` should switch the sidebar to `NewMessagePanel` without navigating away from the current content (e.g., an active chat). This uses SvelteKit's `pushState('', { sidebarPanel: 'new-message' })` to update `page.state` without changing the URL. `DesktopLayout` reads `page.state.sidebarPanel` alongside the URL path to determine which sidebar panel to show. The browser back button automatically pops this state. The `App.PageState` type is augmented in `ui/src/app.d.ts`.

**Add-contact routes are nested under their parent context** (`/new-message/add-contact` and `/settings/profile/add-contact`) so that the correct sidebar panel is shown on desktop based on the URL prefix.

### UI Navigation Map

The app uses SvelteKit file-based routing. On first launch the user sees the Create Profile screen; after creating a profile the home page (`/`) is the root. The theme (Material or iOS) determines whether some actions use buttons/FABs (Material) or navbar links (iOS).

```
Create Profile (first launch only)
  └─ / (Home — chat list)

/ (Home)
  ├─ [avatar] ──────────── /settings
  ├─ [contacts icon] ───── /contacts
  ├─ [new message] ─────── /new-message        (FAB on Material, navbar link on iOS)
  └─ [chat item] ──────── /direct-chats/{agentId}  or  /group-chat/{chatId}

/settings
  ├─ [profile item] ────── /settings/profile
  ├─ [QR icon] ──────────── /settings/profile/add-contact
  └─ [account item] ────── /settings/account

/settings/profile
  ├─ [edit photo] ──────── /settings/profile/edit-photo
  ├─ [name item] ──────── /settings/profile/edit-name
  ├─ [about item] ─────── /settings/profile/edit-about
  └─ [QR code item] ───── /settings/profile/add-contact

/settings/profile/add-contact
  ├─ code tab ──── shows QR + code input
  └─ scan tab ──── camera scanner (mobile only)

/settings/account
  └─ [delete account] ─── confirmation dialog

/new-message
  ├─ [add contact] ────── /new-message/add-contact
  └─ [contact item] ───── /direct-chats/{agentId}

/new-message/add-contact
  ├─ code tab ──── shows QR + code input
  └─ scan tab ──── camera scanner (mobile only)

/new-group
  ├─ step 1: member selection ─── [next] ──► step 2: group info ─── [create]
  └─ step 2 back ──► step 1

/direct-chats/{agentId}
  ├─ [navbar title] ────── /direct-chats/{agentId}/chat-settings
  └─ [back] ────────────── /

/direct-chats/{agentId}/chat-settings
  ├─ [search button] ───── /direct-chats/{agentId}?search=true
  └─ [back] ────────────── /direct-chats/{agentId}

/group-chat/{chatId}
  ├─ [navbar title] ────── /group-chat/{chatId}/info
  └─ [back] ────────────── /
```

### UI Test Utilities

All interactive elements have `data-testid` attributes. Page objects and selector helpers live in `e2e-tests/helpers/`:

- **`e2e-tests/helpers/selectors.ts`** — Exports `tid(id)` which produces a `[data-testid="id"]` CSS selector. Use this everywhere instead of hand-writing the attribute selector.
- **`e2e-tests/helpers/pages/**/*.ts`** — One page object per route (e.g. `home-page.ts`, `direct-chats/direct-chat-page.ts`, `settings/profile/edit-name-page.ts`). Each extends `TestPage`, exposes its elements as `this.agent.$(tid('...'))` fields, and provides a `ready()` method that waits for the page to be interactive.
- **`e2e-tests/helpers/components/*.ts`** — Page objects for reusable components (sheets, banners) that aren't tied to one route.
- **`e2e-tests/helpers/flows/*.ts`** — Multi-page UI flows shared across specs (e.g. `exchange-contacts.ts` walks both agents through navigateToAddContact + enterCode).
- **`e2e-tests/setup/setup-agents.ts`** — `setupAgent('agent1' | 'agent2')` returns an `Agent` (the WDIO browser plus all page-object instances and a few agent-level helpers like `agent.tr`, `agent.checkOverflow`, `agent.setTheme`).

When driving the app via Tauri MCP tools (not via WDIO), use `data-testid` selectors directly. For Konsta `ListInput` components, the `data-testid` lands on the outer `<li>`, so type into `[data-testid="..."] input` (or `textarea` for text areas).

### State Management (packages/stores)

The `packages/stores` package implements a layered reactive state management system using Signalium. It bridges the gap between Svelte components and the Tauri/Rust backend.

**Architecture Layers:**

1. **Client Classes** (`*-client.ts`): Thin wrappers around Tauri `invoke()` calls for backend communication
   ```typescript
   // Example: contacts-client.ts
   export class ContactsClient implements IContactsClient {
     myAgentId(): Promise<AgentId> {
       return invoke('my_agent_id');
     }
     addContact(contactCode: ContactCode): Promise<void> {
       return invoke('add_contact', { contactCode });
     }
   }
   ```

2. **Store Classes** (`*-store.ts`): Reactive state containers that transform raw data into computed/derived state
   ```typescript
   // Example: contacts-store.ts
   export class ContactsStore {
     constructor(
       protected logsStore: LogsStore<Payload>,
       protected devicesStore: DevicesStore,
       public client: IContactsClient,
     ) {}

     // Reactive computed properties using signalium's reactive()
     myProfile = reactive(async () => {
       const myAgentId = await this.myAgentId();
       return await this.profiles(myAgentId);
     });
   }
   ```

3. **LogsStore** (`p2panda/logs-store.ts`): Base store for p2panda operation logs with automatic event subscription
   - Fetches logs via `LogsClient.getLog()` and `getAuthorsForTopic()`
   - Subscribes to `p2panda://new-operation` events for real-time updates
   - Uses `relay()` for cleanup on unsubscribe

**Key Signalium Primitives:**

- `reactive()`: Creates memoized reactive computations that re-run when dependencies change
- `relay()`: Creates reactive values with cleanup/teardown logic (for event subscriptions)
- `ReactivePromise`: Async-aware reactive wrapper that tracks pending/resolved/rejected states
- `watcher()`: Observes reactive values and notifies on changes (used to bridge to Svelte)

**Backend Event Flow:**

1. Rust backend receives new p2panda operations via `notification_rx` channel (`src-tauri/src/lib.rs`)
2. Operations are serialized and emitted as `p2panda://new-operation` Tauri events
3. `TauriLogsClient` listens via `@tauri-apps/api/event.listen()` and invokes registered handlers
4. `LogsStore` updates reactive state, triggering dependent store recomputations

**Svelte Integration:**

Stores are bridged to Svelte's store contract via `ui/src/lib/stores/use-signal.ts`:

```typescript
// useReactivePromise converts Signalium ReactivePromise to Svelte Readable
const myProfile = useReactivePromise(contactsStore.myProfile);

// In Svelte component: use $myProfile with {#await}
{#await $myProfile then profile}
  <span>{profile.name}</span>
{/await}
```

**Store Initialization:**

Stores are instantiated in `ui/src/routes/+layout.svelte` and passed via Svelte context:

```typescript
const logsClient = new TauriLogsClient<TopicId, Payload>();
const logsStore = new LogsStore<Payload>(logsClient);

const devicesStore = new DevicesStore(logsStore, new DevicesClient());
setContext('devices-store', devicesStore);

const contactsStore = new ContactsStore(logsStore, devicesStore, new ContactsClient());
setContext('contacts-store', contactsStore);

const chatsStore = new ChatsStore(logsStore, contactsStore, new ChatsClient());
setContext('chats-store', chatsStore);
```

**Store Composition:**

Stores depend on each other forming a dependency graph:
- `LogsStore` (base) ← `DevicesStore` ← `ContactsStore` ← `ChatsStore`
- Domain-specific stores (e.g., `DirectChatStore`, `GroupChatStore`) are created on-demand with specific parameters

### Data Flow

1. User action in Svelte UI
2. Svelte store calls client function
3. Client invokes Tauri command (crosses JS/Rust boundary)
4. Command handler in src-tauri/commands/ processes request
5. Interacts with Node (dashchat-node crate)
6. Node performs p2panda operations (log operations, sync, discovery)
7. Results returned through Tauri command response
8. Async updates pushed via Tauri events to frontend
9. Frontend stores react to updates and UI re-renders

### P2Panda Integration

The app uses p2panda for:
- Distributed log-based data structures
- End-to-end encryption
- Peer discovery (mDNS)
- Data synchronization between nodes
- Spaces for grouping related data

Core p2panda dependencies (from custom fork):
- p2panda-core: Core types and operations
- p2panda-auth: Authentication
- p2panda-encryption: E2EE
- p2panda-net: Networking layer
- p2panda-sync: Synchronization logic
- p2panda-spaces: Space management
- p2panda-discovery: Peer discovery (mDNS)

## CI

Execute all CI commands inside of the default nix shell with `nix develop`.

## Testing

### Rust Tests
```bash
cargo nextest run
```

Run tests from workspace root. Tests use tokio async runtime.

### Running Tests Against a Deployment Environment

By default all tests use a local (in-memory or in-process) mailbox. Run the same suites against a deployment environment's cloud mailbox by setting `MAILBOX_URL` to that environment's mailbox. The `justfile` loads `.env.${ENV:-development}` via dotenv, so `ENV=<env>` selects the environment (whose `.env.<env>` defines `MAILBOX_URL`):

```bash
ENV=testing just test          # Rust suite
ENV=testing just test e2e      # E2E suite
```

- **Rust**: tests build their mailbox via `TestMailbox::from_env()` (`crates/dashchat-node/src/testing/mailbox.rs`) and register it with `TestNode::add_mailbox(&mb)`. `MAILBOX_URL` unset → a fresh `MemMailbox`; set → a `ToyMailboxClient` for that mailbox, registered the way the production app does it (id from `/health`, address book entry, `/peers/register`). Don't use `MemMailbox::new()` directly in new tests.
- **E2E**: when `MAILBOX_URL` is set, `wdio.conf.ts` skips spawning the local mailbox server and points the agents at it. Specs that drive the mailbox server's lifecycle (suspend/kill) skip themselves via `isRemoteMailbox()`.

Allowed mailbox URLs are allowlisted as regex patterns in the repo-root `allowed-test-mailbox-url-patterns.json`, read by both suites (`TestMailbox::from_env` in Rust and `remoteMailboxUrl()` in `e2e-tests/setup/test-env.ts`); a `MAILBOX_URL` matching none of them fails fast so tests can never hit staging/production.

### Development Testing
Use `pnpm start` to run two instances locally that can communicate with each other over the p2panda network.

### E2E Tests (WebdriverIO)

The `e2e-tests/` package contains automated end-to-end tests using WebdriverIO + `tauri-driver`. Tests launch two built Tauri instances and exercise the full messaging flow (profile creation, contact exchange, messaging).

```bash
# Build the Tauri binary and run the e2e suite (recommended)
just test e2e

# Build the binary only
just test e2e-build

# Build and run a single spec
just test e2e full-flow
```

**Key details:**
- Tests use page objects from `e2e-tests/helpers/pages/`. `setupAgent('agent1')` returns an `Agent` with all page-object instances pre-attached (`agent.homePage`, `agent.directChatPage`, …).
- For DOM-side work that can't be modeled as a click (bulk overflow scans, programmatic event dispatch, test-only file-input injection), tests call `window.__test` functions (registered by `ui/tests/setup-utils.ts`) via `browser.execute()`.
- Two `tauri-driver` instances run on ports 4444 and 4446.
- Launch scripts (`e2e-tests/setup/`) set `DATA_DIR` and `MAILBOX_URL` env vars.
- The binary is built with `--features e2e-tests` to skip single-instance/updater plugins and throttle events.
- Test data is stored in `.dbs/e2e/` and cleaned up after each run.

**E2E coding style:**

- **Navigate via the UI, not `agent.goto()`.** Always walk through the app the way a user would — click back buttons, sidebar links, FABs, list items. `agent.goto()` and `window.__test.goto()` exist as escape hatches only for cases that genuinely cannot be reached by clicking (e.g. simulating a page reload, or the `review-checks` spec that programmatically enumerates every page). If you add a new `goto()` call, leave a comment explaining why the UI path doesn't work.
- **Specs run in narrow (mobile) layout by default.** `setupAgent` forces `agent.setWideScreen(false)` so back buttons (`direct-chat-back`, `offline-back`, …) and FABs render — most of those are gated by `{#if !isWideScreen.value}`. When a spec needs the desktop two-panel layout (e.g. `review-checks` switching combos), call `agent.setWideScreen(true)` in `before()`. Wide-screen mode mounts `ChatListPanel` / `SettingsPanel` / `NewMessagePanel` in the sidebar; some navigation steps that need a back-out in narrow can skip it in wide-screen because the sidebar is always there. The handful of cross-mode helpers (`helpers/review/visit-all-pages.ts`) guard with `if (await page.back.isDisplayed())` to handle both.
- **Use `page.ready()` instead of bare waits after navigation.** Each page object has a `ready()` method that waits for the first stable element on that page. Call it right after the click that triggered the navigation.
- **Only pass custom `waitUntil` / `waitForExist` arguments when strictly necessary.** The default `waitforTimeout` (10s) is correct for incidental waits — animations, navigations, store hydration. Override `timeout` / `interval` / `timeoutMsg` only when (a) the operation genuinely needs longer than 10s (network sync, p2p propagation, connection-state flips that depend on real timeouts) or (b) a custom error message is the only way to diagnose a flake. Don't copy timeouts from neighbouring code without justifying them.
- **One page object per route, one spec per feature.** When you add a new route under `ui/src/routes/`, add a matching page object under `e2e-tests/helpers/pages/` (mirror the route structure: `routes/settings/profile/edit-name/+page.svelte` → `helpers/pages/settings/profile/edit-name-page.ts`) and wire it into `setup-agents.ts`. New UI features must also ship with a spec in `e2e-tests/specs/` covering the happy path.

**REQUIREMENT:** E2E specs must drive the UI via page objects (`agent.homePage.newMessageButton.click()`), not by inlining `document.querySelector` or duplicating selectors. DOM-side helpers that can't be expressed as clicks belong in `ui/tests/setup-utils.ts` under `window.__test`.

**REQUIREMENT:** New UI features must include E2E test coverage in `e2e-tests/specs/`, and every new route must have a corresponding page object under `e2e-tests/helpers/pages/` registered in `setup-agents.ts`.

**REQUIREMENT:** The review-checks E2E test (`e2e-tests/specs/review-checks.spec.ts`) must visit every page in the app. When adding a new page, add it to `e2e-tests/helpers/review/visit-all-pages.ts` so it is covered by the overflow, dark-mode, and RTL checks.

### Backwards Compatibility Tests

The `e2e-tests/compat/` directory contains tests that verify data created by older versions can be read by the current version. This catches breaking changes to the data model before they ship.

```bash
# Run compat test against a specific version tag
cd e2e-tests && bash compat/run.sh v0.10.0

# Test multiple versions
cd e2e-tests && bash compat/run.sh v0.10.0 v0.10.1
```

**How it works:**
1. Builds the current version and the old version (with patches for E2E support)
2. Phase 1 (setup): Creates profiles, contacts, and messages using the old binary
3. Phase 2 (verify): Launches the current binary against the same data and verifies everything persisted
4. Data is stored in `.dbs/compat/` with state saved to `state.json` between phases

**Key files:**
- `compat/run.sh` — Orchestrator script (entry point)
- `compat/wdio.compat.ts` — WDIO config (reads COMPAT_PHASE and COMPAT_BINARY env vars)
- `specs/compat-setup.spec.ts` — Phase 1: create data with old version
- `specs/compat-verify.spec.ts` — Phase 2: verify with current version

### Verifying UI Features

**REQUIREMENT:** Every time you make UI changes, you MUST start the app, visually verify that the feature works correctly and looks polished, and then kill the dev processes when done. Do not skip this step.

1. Use the `start-dev` skill to start the development environment.
2. Connect via `driver_session` and use `webview_screenshot`, `webview_dom_snapshot`, and other Tauri MCP tools to inspect and interact with the UI.
3. Verify that the feature works as expected and the UI is well polished — check layout, spacing, alignment, text, colors, and interactive states.
4. If something looks off, fix it and re-verify.
5. When done, kill all background dev processes (Tauri agents, mailbox server, stores watcher) to free up ports and resources.

### Mobile Virtual Keyboard Handling

The `tauri-plugin-virtual-keyboard` plugin (source: `https://github.com/dash-chat/tauri-plugin-virtual-keyboard`) owns all soft-keyboard integration ([tauri-apps/tauri#10631](https://github.com/tauri-apps/tauri/issues/10631) background). Its contract:

- **The webview never resizes.** The keyboard overlays it; the plugin maintains a `--keyboard-height` CSS variable on `documentElement`, updated per frame during open/close animations (Android: insets animation callback; iOS: display-link interpolation). `MobileLayout` pads its shell by this variable, which reproduces resize semantics declaratively for every screen.
- **Native commands** `hide`/`show` retract/summon the IME at the OS level (`WindowInsetsControllerCompat` on Android, `endEditing` on iOS; `show` is a no-op on iOS and desktop). Exposed via the `tauri-plugin-virtual-keyboard` npm package (`hideKeyboard()`/`showKeyboard()`). Never manipulate the keyboard through DOM focus tricks.
- **Native events** `willShow {height, durationMs}` / `willHide {durationMs}` / `didShow` / `didHide` / `change` feed the guest-js keyboard state, exposed as signalium signals (`keyboard.height/isOpen/reservedHeight`, each with `.value`; signalium is a peer dependency of the npm package). In Svelte, bridge them with `useSignal(() => keyboard.isOpen.value)` from `$lib/stores/use-signal`; read them imperatively via `.value`. `willShow` arrives before the animation with the exact target height.

### iOS Simulator Testing

Testing keyboard behavior and UI interactions in the iOS simulator has inherent limitations due to idb + WKWebView interop issues. **Keyboard behavior is best verified on a real device.**

**What works in the simulator:**
- `idb ui tap --udid <UDID> <x> <y>` can focus *some* WKWebView inputs (e.g. Konsta `ListInput` with `placeholder` prop). Coordinates are in device points (iPhone 16: 393x852).
- Typing via AppleScript `keystroke` when hardware keyboard is connected (toggle with Cmd+Shift+K in Simulator)
- `xcrun simctl pbcopy <UDID>` to set pasteboard content
- `xcrun simctl io <UDID> screenshot <path>` to capture screenshots
- Visual verification of keyboard show/hide (no scrollbar, proper frame resize)

**What doesn't work:**
- `idb ui tap` doesn't reliably reach all WKWebView elements (floating-label Konsta inputs don't respond)
- `idb ui text` doesn't type into WKWebView inputs
- Tapping virtual keyboard keys dismisses the keyboard instead of typing
- `xcrun simctl keyboard input` is not available on iOS 18
- AppleScript `click at` screen coordinates doesn't reach WKWebView content

**Recommended workflow for iOS simulator testing:**
1. Start with `pnpm tauri ios dev "iPhone 16"`
2. Disconnect hardware keyboard (Cmd+Shift+K) to show software keyboard
3. Use `idb ui tap` to focus inputs (works for some elements)
4. Connect hardware keyboard (Cmd+Shift+K) to type via AppleScript
5. Toggle back to verify keyboard visual behavior
6. Use `xcrun simctl io <UDID> screenshot` to capture and inspect state

**iOS app icon note:** iOS icons must have NO alpha channel. The `tauri icon --ios-color` command generates RGBA PNGs (Tauri CLI bug). Fix by stripping alpha from all icons in `src-tauri/gen/apple/Assets.xcassets/AppIcon.appiconset/`.

## Important Notes

- **Log redaction**: The `get_redacted_log` command in `src-tauri/src/commands/logs.rs` strips sensitive data from log files before they are sent as error report attachments. This includes: hex strings, base64 blobs, public key byte arrays, hashes, signatures, device/agent IDs, timestamps, profile fields (name, surname, about), chat message content, and reactions. **When adding any new feature that introduces private or user-generated data, you must also update the redaction patterns in `get_redacted_log` to ensure that data never leaves the device in error reports.**
- **Rust edition**: Uses Rust edition 2021 (src-tauri) and 2024 (dashchat-node)
- **Rust toolchain**: Pinned to stable 1.94.0 via `rust-toolchain.toml` (mobile variants: `rust-toolchain.ios.toml`, `rust-toolchain.android.toml`).
- **Mobile vs Desktop**: Code paths differ for mobile/desktop (check `#[cfg(mobile)]` and `#[cfg(not(mobile))]`)
- **Internationalization**: UI supports multiple languages via Weblate integration

### Notes for edits

- Run `just format` before considering any code ready to commit.
- IDE diagnostics for Rust code will usually be stale and inaccurate while you're changing things. If you can verify that they're up to date, use them, but otherwise prefer running explicit lints/checks.
 
## Build Configuration

### Development
Standard development builds with debug symbols.

### Release
Optimized builds with:
- opt-level 3
- LTO enabled ("fat")
- Single codegen unit
- Panic = abort

## Localization

Translations managed through Weblate: https://hosted.weblate.org/projects/dash-chat
Contact team at hello@dashchat.org to become a translation reviewer.

**IMPORTANT:** Never modify non-English translation files. They are managed exclusively through Weblate and any manual changes will be overwritten. Only the English source strings (`en.json`) should be edited in code.

