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

- Try to remain as simple as possible with your implementations.
- Try to reuse types and functions across the project rather than reimplement them.
- Don't use `any` or `unknown` typescript types. Instead, try to understand the actual typescript types and use them to infer the appropriate data structures and algorithms to use.
- Prefer Tailwind CSS utility classes over custom CSS styles whenever possible. Use inline `class` attributes with Tailwind classes instead of adding styles to `<style>` blocks.

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
cargo test
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
- **e2e-tests/**: WebdriverIO E2E test suite
- **crates/dashchat-node/**: Core p2p backend logic (Rust)
- **crates/mailbox-server/**: HTTP server for offline message storage
- **src-tauri/**: Tauri application wrapper and integration layer
- **site/**: Marketing/download site

### Backend Architecture (Rust)

**Main Components:**

1. **dashchat-node** (`crates/dashchat-node/`):
   - Core p2p networking logic built on p2panda
   - Key modules:
     - `node.rs`: Main Node implementation with p2panda integration
     - `chat.rs` & `contact.rs`: Chat and contact management
     - `spaces.rs` & `topic.rs`: Space and topic abstractions
     - `stores/`: Data persistence layer
     - `polestar/`: Additional p2panda functionality
   - Uses p2panda libraries from custom fork: `https://github.com/maackle/p2panda.git` (branch: dashchat)

2. **src-tauri** (Tauri app layer):
   - `lib.rs`: Application setup, plugin initialization, and node lifecycle
   - `commands/`: Tauri command handlers that bridge frontend to backend:
     - `logs.rs`: Operation log queries
     - `profile.rs`: User profile management
     - `contacts.rs`: Contact management
     - `devices.rs`: Device management
     - `chats.rs` & `group_chat.rs`: Chat functionality
   - `push_notifications.rs`: Mobile push notification handling
   - `menu.rs`: Desktop menu configuration
   - `utils.rs`: Shared utilities

3. **mailbox-server** (`crates/mailbox-server/`):
   - Standalone HTTP server for storing/retrieving encrypted message blobs
   - Built with Axum web framework and redb embedded database
   - Key modules:
     - `lib.rs`: App initialization, routing, and database setup
     - `store_blobs.rs`: POST `/blobs/store` endpoint for storing blobs
     - `get_blobs.rs`: POST `/blobs/get` endpoint for retrieving blobs with sync support
     - `cleanup.rs`: Background task that deletes messages older than 7 days
     - `blob.rs`: Base64-encoded binary data wrapper
   - Data model:
     - Key format: `topic_id:log_id:sequence_number:uuid_v7`
     - Blobs organized by topic → log → sequence number hierarchy
     - UUID v7 suffix enables time-based cleanup
   - Features bidirectional sync: returns missing blobs to client AND requests blobs the server is missing
   - Run with: `cargo run --bin mailbox-server -- --db-path <path> --addr <addr>`

**Key Backend Patterns:**
- Node managed as Tauri state (accessed via `app.state::<Node>()`)
- Async notification channel from Node to frontend via Tauri events
- All backend commands are async and return `Result<T, String>`
- Uses p2panda's operation-based data model with CBOR encoding

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
- Tauri commands invoked via `invoke()` from `@tauri-apps/api`
- UI built with Konsta UI components (mobile-first design)
- Internationalization using @inlang/paraglide-js
- Image compression before upload
- **iOS theme action buttons**: In the iOS theme, all primary action buttons (Save, Done, Create, Add, Next) must appear as a `<Link>` in the Navbar's `right` snippet — never as a bottom FAB. The bottom FAB (`class="fixed-action-btn"`) is Material-only. Use `{#if theme === 'ios'}` in the navbar right snippet and `{#if theme === 'material'}` around the FAB. Apply disabled styling via `rightClass="ios-right-disabled"` on the Navbar (defined in `app.css`).

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

All interactive elements have `data-testid` attributes. The selector registry and page objects live in `ui/tests/`:

- **`ui/tests/selectors.ts`** — Single source of truth for all `data-testid` selectors, organized by page. Use `S.pageName.elementName` to get a CSS selector like `[data-testid="page-element"]`.
- **`ui/tests/pages/*.ts`** — Page object modules exporting selectors, interaction descriptors, and assertion scripts for each page.
- **`ui/tests/flows/*.ts`** — Multi-step workflow descriptors (profile creation, contact exchange, send message).

When driving the app via Tauri MCP tools, always use `data-testid` selectors instead of CSS class selectors. For Konsta `ListInput` components, the `data-testid` lands on the outer `<li>`, so type into `[data-testid="..."] input` (or `textarea` for text areas).

Reference `ui/tests/selectors.ts` for the full list of available selectors.

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
cargo test
```

Run tests from workspace root. Tests use tokio async runtime.

### Development Testing
Use `pnpm start` to run two instances locally that can communicate with each other over the p2panda network.

### E2E Tests (WebdriverIO)

The `e2e-tests/` package contains automated end-to-end tests using WebdriverIO + `tauri-driver`. Tests launch two built Tauri instances and exercise the full messaging flow (profile creation, contact exchange, messaging).

```bash
# Build the app first (debug, no-bundle)
pnpm tauri build --debug --no-bundle

# Run E2E tests (builds automatically unless SKIP_BUILD=1)
cd e2e-tests && pnpm test

# Skip the build step (useful when binary is already built)
cd e2e-tests && SKIP_BUILD=1 pnpm test
```

**Key details:**
- Tests call `window.__test` functions (registered by `ui/tests/setup-utils.ts`) via `browser.execute()`
- Two `tauri-driver` instances run on ports 4444 and 4446
- Launch scripts (`e2e-tests/scripts/`) set `DATA_DIR` and `MAILBOX_URL` env vars
- The binary is built with `--features e2e-tests` to skip single-instance/updater plugins and throttle events
- Test data is stored in `.dbs/e2e/` and cleaned up after each run

**REQUIREMENT:** New UI features must include E2E test coverage in `e2e-tests/specs/`.

**REQUIREMENT:** The review-checks E2E test (`e2e-tests/specs/review-checks.spec.ts`) must visit every page in the app. When adding a new page, add it to `ui/tests/review/visit-all-pages.ts` so it is covered by the overflow, dark-mode, and RTL checks.

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

## Platform Support

- **Desktop**: Linux, macOS, Windows (via Tauri)
- **Mobile**: Android and iOS support
  - Android-specific: barcode scanner, push notifications
  - iOS-specific: barcode scanner, push notifications, safe area insets

### iOS Virtual Keyboard Handling

The `tauri-plugin-virtual-keyboard-padding` plugin handles iOS keyboard behavior in WKWebView. Without it, iOS shows a scrollable white gap behind the keyboard.

**How the plugin works:**
1. Removes WKWebView's built-in keyboard notification observers (prevents auto contentInset/contentOffset adjustments)
2. Clamps `scrollView.contentOffset` to `.zero` via `UIScrollViewDelegate`
3. Resizes WKWebView frame by keyboard height so focused inputs remain visible
4. Injects CSS to override `.min-h-screen`/`.h-screen` with pixel values (100vh doesn't update on frame resize)
5. Auto-detects background color from rendered `.k-page` element via JavaScript and applies to native view hierarchy (prevents color flash during animation)
6. Removes the "Done" toolbar by swizzling `WKContentView.inputAccessoryView`

**Plugin source:** `tauri-plugin-virtual-keyboard-padding` (sibling repo). `Cargo.toml` uses git URL; for local dev change to `path = "../../tauri-plugin-virtual-keyboard-padding"`.

**CSS requirement:** `html` and `body` must have `background-color: transparent !important` (set in `app.css`) so the native background color shows through during keyboard animation.

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
- **P2panda fork**: This project uses a custom fork of p2panda. Do not update p2panda dependencies without checking compatibility.
- **Rust edition**: Uses Rust edition 2021 (src-tauri) and 2024 (dashchat-node)
- **Nightly features**: dashchat-node uses `#![feature(bool_to_result)]`
- **Mobile vs Desktop**: Code paths differ for mobile/desktop (check `#[cfg(mobile)]` and `#[cfg(not(mobile))]`)
- **Internationalization**: UI supports multiple languages via Weblate integration

## Releasing

Use `scripts/release.sh` to cut a new release:
```bash
./scripts/release.sh 0.11.0
```

This updates the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and the download links in `packages/site/index.html`, then commits, tags (`vX.Y.Z`), and pushes.

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

