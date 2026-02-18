# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Dash Chat is an end-to-end encrypted messenger built with Svelte 5 (frontend) and Rust/Tauri (backend), using p2panda for peer-to-peer communication. The application works both with and without internet connectivity.

**Current Status**: Pre-alpha, being rebuilt on top of p2panda.

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
```

## Architecture

### Monorepo Structure

This is a pnpm workspace with multiple packages:
- **ui/**: Svelte 5 + TypeScript frontend (SvelteKit application)
- **packages/stores/**: Shared TypeScript stores for state management
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
  ├─ [QR icon] ──────────── /add-contact
  └─ [account item] ────── /settings/account

/settings/profile
  ├─ [edit photo] ──────── /settings/profile/edit-photo
  ├─ [name item] ──────── /settings/profile/edit-name
  ├─ [about item] ─────── /settings/profile/edit-about
  └─ [QR code item] ───── /add-contact

/settings/account
  └─ [delete account] ─── confirmation dialog

/contacts
  └─ [add icon] ────────── /add-contact

/add-contact
  ├─ code tab ──── shows QR + code input
  └─ scan tab ──── camera scanner (mobile only)

/new-message
  └─ [contact item] ───── /direct-chats/{agentId}

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

## Important Notes

- **P2panda fork**: This project uses a custom fork of p2panda. Do not update p2panda dependencies without checking compatibility.
- **Rust edition**: Uses Rust edition 2021 (src-tauri) and 2024 (dashchat-node)
- **Nightly features**: dashchat-node uses `#![feature(bool_to_result)]`
- **Mobile vs Desktop**: Code paths differ for mobile/desktop (check `#[cfg(mobile)]` and `#[cfg(not(mobile))]`)
- **Internationalization**: UI supports multiple languages via Weblate integration

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

