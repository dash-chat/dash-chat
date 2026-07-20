# Block-contact UI design

## Goal

Finish the user-facing UI for blocking/unblocking contacts on top of the
existing backend + store plumbing (`blockContact`/`unblockContact` clients,
`blockedContactAgentIds` reactive set). Three gaps remain:

1. Blocking and unblocking a contact must go through a confirmation dialog.
2. On the contact-request screen the red **Reject** button becomes **Block**,
   using the same block confirmation dialog.
3. A blocked contact's name shows a ⊘ ("no") icon in the chat list and in the
   direct-chat title.

Plus one supporting piece the above implies: a blocked direct chat must replace
the composer with a blocked banner (we intend to grow that banner later).

Scope note: blocking is a **direct-chat** concern only. Group rows/members are
out of scope.

## Existing pieces (already present, do not rebuild)

- `contactsStore.client.blockContact(agentId)` / `unblockContact(agentId)`.
- `contactsStore.blockedContactAgentIds` — reactive `Set<AgentId>`.
- `mdiCancel` is the ⊘ icon already used for the block toggle.
- Konsta `Dialog` / `DialogButton` — pattern already used by the
  accept/reject dialogs in `direct-chats/[agentId]/+page.svelte`.
- Konsta `ListItem` accepts a **snippet** for its `title` prop
  (`{#if typeof title === 'function'}`), so a list title can contain an icon.

## Components & changes

### 1. `BlockContactDialog.svelte` (new)

Location: `ui/src/lib/components/contacts/BlockContactDialog.svelte`.

Reused at all three toggle sites, so it lives as one component.

Props:

- `opened: boolean`
- `name: string`
- `blocked: boolean` — `true` means the contact is currently blocked, so the
  dialog offers **Unblock**; `false` offers **Block**.
- `onConfirm: () => void`
- `onClose: () => void`

Renders a Konsta `Dialog` (title + body + Cancel/confirm buttons):

- Block mode: title `m.blockContactTitle({ name })`, body
  `m.blockContactDescription()`, confirm label `m.block()`.
- Unblock mode: title `m.unblockContactTitle({ name })`, body
  `m.unblockContactDescription()`, confirm label `m.unblock()`.
- Confirm button `data-testid="block-contact-confirm"`.

The dialog only renders confirmation UI; each call site owns the actual
block/unblock call and any navigation.

### 2. i18n (`ui/messages/en.json`, English source only)

Add:

- `blockContactTitle`: `"Block {{name}}?"`
- `blockContactDescription`: `"Blocked people won't be able to send you messages."`
- `unblockContactTitle`: `"Unblock {{name}}?"`
- `unblockContactDescription`: `"You will be able to message each other. Any messages they may have sent while they were blocked will not be shown."`
- `youBlockedThisPerson`: `"You blocked this person."` (direct-chat blocked banner)

Remove the stale prototype key `blockContactConfirm` (currently unused after
this change). Reuse existing `block` / `unblock`.

### 3. Toggle sites use the dialog

- **`direct-chats/[agentId]/chat-settings/+page.svelte`**: the block/unblock
  `ListItem` now opens `BlockContactDialog` instead of calling `toggleBlock`
  directly. Confirm → `blockContact` / `unblockContact`.
- **`layout/NewMessagePanel.svelte`**: the contact action-menu toggle opens the
  dialog instead of acting immediately. It already tracks `menuIsBlocked` /
  `menuFor` (agentId + profile), which feed the dialog's `blocked` / `name`.

### 4. Contact-request banner → Block

In `direct-chats/[agentId]/+page.svelte`:

- Replace the red **Reject** button (`direct-chat-reject-btn`) with a **Block**
  button (`m.block()`, `data-testid="direct-chat-block-btn"`) that opens
  `BlockContactDialog` in block mode.
- On confirm: call `contactsStore.client.blockContact(agentId)` and **stay on
  the chat** (no navigation). Blocking invalidates the peer's request op, so the
  request banner disappears and the blocked banner (below) takes its place.
- Remove the now-unused reject dialog and `rejectContactRequest` handler from
  this screen. (`rejectContactRequest` on the client/backend stays; it is simply
  no longer wired to this button.)

### 5. Blocked banner in the direct chat

Add `isBlocked` to the page (derive from `contactsStore.blockedContactAgentIds`
and `agentId`, same as chat-settings does).

Bottom-of-chat branch order:

1. `isPendingChat` → existing waiting-for-profile header, no input.
2. **`isBlocked`** → blocked banner: ⊘ (`mdiCancel`) + `m.youBlockedThisPerson()`
   and an **Unblock** button that opens `BlockContactDialog` in unblock mode
   (confirm → `unblockContact`). `data-testid="direct-chat-blocked-banner"`.
3. `contactRequest` → existing request banner (now with the Block button).
4. else → `MessageComposer`.

`isBlocked` and `contactRequest` are mutually exclusive in practice (a block
invalidates the request op), so ordering blocked before request is safe.

### 6. ⊘ icon next to blocked names

- **Chat list** (`ChatSummary.svelte` + `AllChats.svelte`): `AllChats` reads
  `contactsStore.blockedContactAgentIds` and passes
  `blocked={summary.type === 'DirectChat' && blockedSet.has(summary.chatId)}`
  to `ChatSummary` (for a direct chat `summary.chatId` **is** the agentId).
  `ChatSummary` renders its title via a `title` snippet that shows a small muted
  ⊘ before the name when `blocked`.
- **Direct-chat title** (`AvatarWithName.svelte`): add a `blocked` prop; render
  the same muted ⊘ before the name. The page passes `blocked={isBlocked}`.

Icon styling: small, muted (matches the quiet/secondary text treatment), placed
before the name using logical spacing (`me-*`).

## Testing

- Page objects: add the dialog + button test IDs to
  `helpers/pages/direct-chats/chat-settings-page.ts`,
  `helpers/pages/direct-chats/direct-chat-page.ts`, and the new-message /
  NewMessagePanel page object.
- New spec `e2e-tests/specs/block-contact.spec.ts` (happy path):
  1. From chat-settings, tap Block → dialog appears → confirm → ⊘ shows in the
     direct-chat title and the chat-list row, and the blocked banner replaces
     the composer.
  2. Tap Unblock (banner or chat-settings) → dialog → confirm → ⊘ and banner
     gone, composer back.
  3. Block from a contact request → dialog → confirm → stays on chat, blocked
     banner shown.
- Add the block-contact flow / any new visible page state to
  `helpers/review/visit-all-pages.ts` only if a new route is introduced (none
  is — all changes are on existing routes), so this is likely a no-op there.

## Out of scope

- Blocking group members.
- A dedicated "Blocked users" settings list (Signal has one; not requested).
- Report/spam flows.
- Changes to reject/block backend semantics.
