# Block-contact UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add confirmation dialogs, a "Block" contact-request action, an in-chat blocked banner, and a ⊘ indicator next to blocked direct-chat contacts, on top of the existing block/unblock backend + store plumbing.

**Architecture:** One shared `BlockContactDialog.svelte` (Konsta `Dialog`) drives every block/unblock confirmation. Call sites (chat-settings, NewMessagePanel action menu, contact-request banner) own the actual `blockContact`/`unblockContact` call. Blocked state is read from the existing `contactsStore.blockedContactAgentIds` reactive `Set<AgentId>`; for a direct chat the `chatId` **is** the agentId.

**Tech Stack:** Svelte 5 (runes), Konsta UI, `@mdi/js` (`mdiCancel`), paraglide i18n (`$lib/paraglide/messages.js`), WebdriverIO E2E.

## Global Constraints

- All Tauri commands via `invokeAfterSetup()` — already true for `blockContact`/`unblockContact`; do not add new commands.
- Use logical CSS properties only (`me-`, `ms-`, `ps-`, `text-start`, …). Never `ml-`/`mr-`/`left`/`right`.
- Never compare numbers as booleans — use `set.size > 0`, `x !== 0`, etc. Use `!!x` only for non-numeric coercion.
- Edit only `ui/messages/en.json`; never touch other locale files.
- Write very few comments (none by default).
- Every new interactive element needs a `data-testid`.
- A component's root owns no outer margin; spacing between siblings is the parent's `gap`.

---

### Task 1: `BlockContactDialog.svelte` + i18n keys

**Files:**
- Modify: `ui/messages/en.json:9` (replace `blockContactConfirm`; add title/body keys + banner text)
- Create: `ui/src/lib/components/contacts/BlockContactDialog.svelte`

**Interfaces:**
- Produces: `BlockContactDialog` with props `{ opened: boolean; name: string; blocked: boolean; onConfirm: () => void; onClose: () => void }`. Confirm button `data-testid="block-contact-confirm"`. Renders nothing actionable itself beyond firing `onConfirm`/`onClose`.
- Produces (i18n): `m.blockContactTitle({ name })`, `m.blockContactDescription()`, `m.unblockContactTitle({ name })`, `m.unblockContactDescription()`, `m.youBlockedThisPerson()`. Reuses existing `m.block()`, `m.unblock()`, `m.cancel()`.

- [ ] **Step 1: Replace the stale key and add new keys in `en.json`**

Replace line 9 (`"blockContactConfirm": "Block {{name}}? They won't be able to message you.",`) with:

```json
	"blockContactTitle": "Block {{name}}?",
	"blockContactDescription": "Blocked people won't be able to send you messages.",
	"unblockContactTitle": "Unblock {{name}}?",
	"unblockContactDescription": "You will be able to message each other. Any messages they may have sent while they were blocked will not be shown.",
	"youBlockedThisPerson": "You blocked this person.",
```

(Keep the existing `"blocked": "Blocked",` line that follows.)

- [ ] **Step 2: Create the dialog component**

`ui/src/lib/components/contacts/BlockContactDialog.svelte`:

```svelte
<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Dialog, DialogButton } from 'konsta/svelte';

	let {
		opened,
		name,
		blocked,
		onConfirm,
		onClose,
	}: {
		opened: boolean;
		name: string;
		blocked: boolean;
		onConfirm: () => void;
		onClose: () => void;
	} = $props();
</script>

<Dialog
	{opened}
	onBackdropClick={onClose}
	title={blocked
		? m.unblockContactTitle({ name })
		: m.blockContactTitle({ name })}
>
	<span>
		{blocked ? m.unblockContactDescription() : m.blockContactDescription()}
	</span>
	{#snippet buttons()}
		<DialogButton onClick={onClose}>{m.cancel()}</DialogButton>
		<DialogButton data-testid="block-contact-confirm" onClick={onConfirm}>
			{blocked ? m.unblock() : m.block()}
		</DialogButton>
	{/snippet}
</Dialog>
```

- [ ] **Step 3: Type-check**

Run (from `ui/`): `pnpm check`
Expected: no new errors referencing `BlockContactDialog.svelte` or the new `m.*` keys. (Paraglide regenerates message functions on build/check; if `m.blockContactTitle` is "not found", run `pnpm --filter dash-chat-ui build` once or restart the dev server to regenerate — the keys exist in `en.json`.)

- [ ] **Step 4: Commit**

```bash
git add ui/messages/en.json ui/src/lib/components/contacts/BlockContactDialog.svelte
git commit -m "Add BlockContactDialog and block/unblock copy"
```

---

### Task 2: Wire dialog into chat-settings

**Files:**
- Modify: `ui/src/routes/direct-chats/[agentId]/chat-settings/+page.svelte`

**Interfaces:**
- Consumes: `BlockContactDialog` (Task 1). Existing `isBlocked`, `contactsStore`, `agentId`, `peerProfile`.

- [ ] **Step 1: Import the dialog and add dialog state**

In the `<script>`, add import alongside the other component imports:

```ts
	import BlockContactDialog from '$lib/components/contacts/BlockContactDialog.svelte';
```

Add state near `let showPeerProfile = $state(false);`:

```ts
	let showBlockDialog = $state(false);
```

- [ ] **Step 2: Change the toggle to open the dialog; add a confirm handler**

Replace the existing `toggleBlock` function:

```ts
	async function toggleBlock() {
		if (isBlocked) {
			await contactsStore.client.unblockContact(agentId);
		} else {
			await contactsStore.client.blockContact(agentId);
		}
	}
```

with:

```ts
	async function confirmBlockToggle() {
		showBlockDialog = false;
		if (isBlocked) {
			await contactsStore.client.unblockContact(agentId);
		} else {
			await contactsStore.client.blockContact(agentId);
		}
	}
```

Change the block `ListItem`'s `onClick={toggleBlock}` to:

```svelte
						onClick={() => (showBlockDialog = true)}
```

- [ ] **Step 3: Render the dialog**

Inside `{#await $peerProfile}` … `{:then profile}`, next to the existing `PeerProfileSheet` (still inside the `{#if profile}` block so `profile.name` is defined), add:

```svelte
			<BlockContactDialog
				opened={showBlockDialog}
				name={profile.name}
				blocked={isBlocked}
				onConfirm={confirmBlockToggle}
				onClose={() => (showBlockDialog = false)}
			/>
```

- [ ] **Step 4: Type-check and visually verify**

Run (from `ui/`): `pnpm check` → no new errors.
Then per CLAUDE.md, verify in the running app (Task 6 covers automated coverage): tapping Block on chat-settings shows the dialog; Cancel dismisses; Block confirms and the row flips to "Unblock".

- [ ] **Step 5: Commit**

```bash
git add ui/src/routes/direct-chats/[agentId]/chat-settings/+page.svelte
git commit -m "Confirm block/unblock from chat settings"
```

---

### Task 3: Wire dialog into NewMessagePanel action menu

**Files:**
- Modify: `ui/src/lib/components/layout/NewMessagePanel.svelte`

**Interfaces:**
- Consumes: `BlockContactDialog` (Task 1). Existing `menuFor` (`{ agentId, profile } | null`), `menuIsBlocked`, `toggleBlock`.

- [ ] **Step 1: Import the dialog**

Add to the script imports:

```ts
	import BlockContactDialog from '$lib/components/contacts/BlockContactDialog.svelte';
```

- [ ] **Step 2: Split "open dialog" from "perform toggle"**

Add dialog state near `let menuIsBlocked = $state(false);`:

```ts
	let showBlockDialog = $state(false);
	let dialogFor = $state<{ agentId: AgentId; profile: Profile } | null>(null);
```

Replace `toggleBlock` and change the action button to open the dialog. Replace:

```ts
	async function toggleBlock() {
		if (!menuFor) return;
		const { agentId } = menuFor;
		menuFor = null;
		if (menuIsBlocked) {
			await contactsStore.client.unblockContact(agentId);
		} else {
			await contactsStore.client.blockContact(agentId);
		}
	}
```

with:

```ts
	function requestBlockToggle() {
		if (!menuFor) return;
		dialogFor = menuFor;
		showBlockDialog = true;
		menuFor = null;
	}

	async function confirmBlockToggle() {
		if (!dialogFor) return;
		const { agentId } = dialogFor;
		showBlockDialog = false;
		if (menuIsBlocked) {
			await contactsStore.client.unblockContact(agentId);
		} else {
			await contactsStore.client.blockContact(agentId);
		}
		dialogFor = null;
	}
```

Change the `ActionsButton`'s `onClick={toggleBlock}` to `onClick={requestBlockToggle}`.

- [ ] **Step 3: Render the dialog after the `<Actions>` block**

Immediately after the closing `</Actions>` tag (before `<style>`):

```svelte
{#if dialogFor}
	<BlockContactDialog
		opened={showBlockDialog}
		name={fullName(dialogFor.profile)}
		blocked={menuIsBlocked}
		onConfirm={confirmBlockToggle}
		onClose={() => {
			showBlockDialog = false;
			dialogFor = null;
		}}
	/>
{/if}
```

- [ ] **Step 4: Type-check**

Run (from `ui/`): `pnpm check` → no new errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/layout/NewMessagePanel.svelte
git commit -m "Confirm block/unblock from new-message contact menu"
```

---

### Task 4: Contact-request "Block" button + in-chat blocked banner

**Files:**
- Modify: `ui/src/routes/direct-chats/[agentId]/+page.svelte`

**Interfaces:**
- Consumes: `BlockContactDialog` (Task 1); existing `contactsStore`, `agentId`, `contactRequest`, `isPendingChat`, `profile`.
- Produces: `isBlocked` (derived `boolean`) on this page, consumed by Task 5 for the title icon.

- [ ] **Step 1: Imports, blocked state, dialog state**

Add imports:

```ts
	import BlockContactDialog from '$lib/components/contacts/BlockContactDialog.svelte';
	import { mdiCancel } from '@mdi/js';
```

(`mdiCancel` joins the existing `@mdi/js` import block — add it to that list rather than a second import if you prefer; either compiles.)

After `const contactsStore: ContactsStore = getContext('contacts-store');`, add:

```ts
	const blockedAgentIds = useReactiveValue(
		contactsStore.blockedContactAgentIds,
	);
	const isBlocked = $derived(($blockedAgentIds ?? new Set()).has(agentId));
```

Add dialog state near `let showRejectDialog = $state(false);`:

```ts
	let showBlockDialog = $state(false);
```

- [ ] **Step 2: Block/unblock confirm handler**

The same dialog is opened from the request banner's Block button (`isBlocked` is
`false` → blocks) and from the blocked banner's Unblock button (`isBlocked` is
`true` → unblocks), so the handler branches on `isBlocked`. Add near
`rejectContactRequest`:

```ts
	async function confirmBlock() {
		showBlockDialog = false;
		try {
			if (isBlocked) {
				await contactsStore.client.unblockContact(agentId);
			} else {
				await contactsStore.client.blockContact(agentId);
			}
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
```

- [ ] **Step 3: Replace the request banner's "Reject" button with "Block"**

In the `{:else if contactRequest}` banner (~line 787), replace the reject `Button`:

```svelte
									<Button
										class="neutral-tonal-button text-red-500 flex-1"
										rounded
										tonal
										data-testid="direct-chat-reject-btn"
										onClick={() => (showRejectDialog = true)}
										>{m.reject()}</Button
									>
```

with:

```svelte
									<Button
										class="neutral-tonal-button text-red-500 flex-1"
										rounded
										tonal
										data-testid="direct-chat-block-btn"
										onClick={() => (showBlockDialog = true)}
										>{m.block()}</Button
									>
```

Leave the Accept button, the reject `Dialog`, `showRejectDialog`, and `rejectContactRequest` untouched (dead-but-retained; cleanup is a deferred follow-up).

- [ ] **Step 4: Add the blocked banner branch**

The bottom-bar conditional currently reads `{#if searchMode}` … `{:else if isPendingChat}` … `{:else if contactRequest}` … `{:else}` (composer). Insert a new branch **before** `{:else if contactRequest}` (so it sits right after the `isPendingChat` block's closing `{:else if isPendingChat}` … `</div>`):

```svelte
						{:else if isBlocked}
							<div class="pb-safe bg-page-surface">
								<div
									class="mx-4 border-t border-gray-300 dark:border-gray-600"
									style="margin: 0 auto"
								></div>
								<div
									class="flex flex-col items-center gap-3 px-6 py-3"
									data-testid="direct-chat-blocked-banner"
								>
									<p
										class="flex items-center gap-2 text-center text-sm text-gray-600 dark:text-gray-400"
									>
										<wa-icon
											class="small-icon quiet shrink-0"
											src={wrapPathInSvg(mdiCancel)}
										></wa-icon>
										{m.youBlockedThisPerson()}
									</p>
									<Button
										class="neutral-tonal-button flex-1"
										rounded
										tonal
										data-testid="direct-chat-unblock-btn"
										onClick={() => (showBlockDialog = true)}
										>{m.unblock()}</Button
									>
								</div>
							</div>
```

- [ ] **Step 5: Render the block dialog**

Alongside the existing accept/reject dialogs, but NOT gated by `{#if contactRequest}` (blocking applies without a pending request). Place it just after the `{#if contactRequest}` … `{/if}` dialog block (~line 660), inside `{#if profile}` so `profile.name` is available — mirror how the page already guards `profile`. Since the dialogs sit inside the scroll page where `profile` is in scope from `{#await $peerProfile then profile}`:

```svelte
						<BlockContactDialog
							opened={showBlockDialog}
							name={profile ? profile.name : ''}
							blocked={isBlocked}
							onConfirm={confirmBlock}
							onClose={() => (showBlockDialog = false)}
						/>
```

`blocked={isBlocked}` makes the dialog show Unblock copy when opened from the
blocked banner and Block copy from the request banner; `confirmBlock` (Step 2)
already routes to the matching client call.

- [ ] **Step 6: Type-check and verify branch order**

Run (from `ui/`): `pnpm check` → no new errors. Confirm the conditional chain reads: `searchMode` → `isPendingChat` → `isBlocked` → `contactRequest` → composer.

- [ ] **Step 7: Commit**

```bash
git add ui/src/routes/direct-chats/[agentId]/+page.svelte
git commit -m "Block from contact request and show blocked banner in chat"
```

---

### Task 5: ⊘ indicator next to blocked direct-chat names

**Files:**
- Modify: `ui/src/lib/components/profiles/AvatarWithName.svelte`
- Modify: `ui/src/routes/direct-chats/[agentId]/+page.svelte` (pass `blocked` to title)
- Modify: `ui/src/lib/components/chats/ChatSummary.svelte`
- Modify: `ui/src/lib/components/chats/AllChats.svelte`

**Interfaces:**
- Consumes: `isBlocked` from Task 4 (page); `contactsStore.blockedContactAgentIds`.
- Produces: `AvatarWithName` gains `blocked?: boolean`; `ChatSummary` gains `blocked?: boolean`.

- [ ] **Step 1: Add `blocked` to `AvatarWithName`**

Replace `ui/src/lib/components/profiles/AvatarWithName.svelte` contents:

```svelte
<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { fullName, type Profile } from 'dash-chat-stores';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCancel } from '@mdi/js';

	let {
		profile,
		nameTestId,
		blocked = false,
	}: { profile: Profile; nameTestId?: string; blocked?: boolean } = $props();
</script>

<span class="flex w-full min-w-0 flex-row items-center gap-2">
	<span class="shrink-0">
		<Avatar
			image={profile.avatar}
			initials={profile.name.slice(0, 2)}
			size="2.5rem"
		/>
	</span>
	<span
		class="flex min-w-0 flex-1 flex-row items-center gap-1"
		data-testid={nameTestId}
	>
		{#if blocked}
			<wa-icon
				class="small-icon quiet shrink-0"
				src={wrapPathInSvg(mdiCancel)}
				data-testid="blocked-name-icon"
			></wa-icon>
		{/if}
		<span class="truncate">{fullName(profile)}</span>
	</span>
</span>
```

(The `nameTestId` moves to the row so the existing `direct-chat-peer-name` selector still resolves and now wraps icon + name.)

- [ ] **Step 2: Pass `blocked` from the direct-chat title**

In `ui/src/routes/direct-chats/[agentId]/+page.svelte`, the title `AvatarWithName` (~line 380):

```svelte
											<AvatarWithName
												{profile}
												blocked={isBlocked}
												nameTestId="direct-chat-peer-name"
											/>
```

- [ ] **Step 3: Add `blocked` to `ChatSummary` and render an icon title**

In `ui/src/lib/components/chats/ChatSummary.svelte`:

Add imports:

```ts
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCancel } from '@mdi/js';
```

Change props:

```ts
	let {
		summary,
		active,
		blocked = false,
	}: { summary: ChatSummary; active: boolean; blocked?: boolean } = $props();
```

Replace the `title={...}` prop on `TitleTruncatedListItem` with a `title` snippet. Remove the `title=` and `titleWrapClass=` attributes and instead add a snippet child:

```svelte
<TitleTruncatedListItem
	link
	class={active ? 'active' : ''}
	linkProps={{ href: chatHref(summary) }}
	chevron={false}
	data-testid="all-chats-row"
>
	{#snippet title()}
		<span
			class="flex min-w-0 flex-row items-center gap-1 {summary.waitingForProfile
				? 'quiet'
				: ''}"
		>
			{#if blocked}
				<wa-icon
					class="small-icon quiet shrink-0"
					src={wrapPathInSvg(mdiCancel)}
					data-testid="blocked-row-icon"
				></wa-icon>
			{/if}
			<span class="truncate"
				>{summary.waitingForProfile
					? m.waitingForProfile()
					: summary.name}</span
			>
		</span>
	{/snippet}
```

Keep the existing `media`, `after`, and `subtitle` snippets unchanged.

- [ ] **Step 4: Feed `blocked` from `AllChats`**

In `ui/src/lib/components/chats/AllChats.svelte`:

Add to the script (import type + reactive set):

```ts
	import { useReactiveValue } from '$lib/stores/use-signal';
	import type { AgentId } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import type { ContactsStore } from 'dash-chat-stores';
```

(`getContext` and `useReactivePromise` are already imported — merge, don't duplicate. Add only the missing `useReactiveValue`, `ContactsStore`, `AgentId`.)

```ts
	const contactsStore: ContactsStore = getContext('contacts-store');
	const blockedAgentIds = useReactiveValue(
		contactsStore.blockedContactAgentIds,
	);
```

Update the render to pass `blocked`:

```svelte
				{#each summaries as summary}
					{@const blockedSet = $blockedAgentIds ?? new Set<AgentId>()}
					<ChatSummary
						{summary}
						active={isActive(summary)}
						blocked={summary.type === 'DirectChat' &&
							blockedSet.has(summary.chatId)}
					/>
				{/each}
```

- [ ] **Step 5: Type-check and visually verify**

Run (from `ui/`): `pnpm check` → no new errors.
Visually: block a contact; confirm the ⊘ shows before their name in the chat-list row and the direct-chat title, and NOT next to group rows.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/profiles/AvatarWithName.svelte ui/src/routes/direct-chats/[agentId]/+page.svelte ui/src/lib/components/chats/ChatSummary.svelte ui/src/lib/components/chats/AllChats.svelte
git commit -m "Show blocked icon next to direct-chat contact names"
```

---

### Task 6: E2E page objects + block-contact spec

**Files:**
- Modify: `e2e-tests/helpers/pages/direct-chats/direct-chat-page.ts`
- Modify: `e2e-tests/helpers/pages/direct-chats/chat-settings-page.ts`
- Modify: `e2e-tests/helpers/pages/home-page.ts` (add blocked-row-icon accessor)
- Create: `e2e-tests/specs/block-contact.spec.ts`

**Interfaces:**
- Consumes test IDs produced by Tasks 1–5: `block-contact-confirm`, `direct-chat-block-btn`, `direct-chat-unblock-btn`, `direct-chat-blocked-banner`, `chat-settings-block-toggle` (existing), `blocked-name-icon`, `blocked-row-icon`.

- [ ] **Step 1: Extend `DirectChatPage`**

Add fields near the existing `rejectButton`:

```ts
	blockButton = this.el(tid('direct-chat-block-btn'));
	unblockButton = this.el(tid('direct-chat-unblock-btn'));
	blockedBanner = this.el(tid('direct-chat-blocked-banner'));
	blockConfirm = this.el(tid('block-contact-confirm'));
	blockedNameIcon = this.el(tid('blocked-name-icon'));
```

- [ ] **Step 2: Extend `ChatSettingsPage`**

```ts
	blockToggle = this.el(tid('chat-settings-block-toggle'));
	blockConfirm = this.el(tid('block-contact-confirm'));
```

- [ ] **Step 3: Add a blocked-row accessor to `HomePage`**

Near `chatRow`:

```ts
	blockedRowIcon = this.el(tid('blocked-row-icon'));
```

- [ ] **Step 4: Write the spec**

`e2e-tests/specs/block-contact.spec.ts`:

```ts
import { setupAgent, type Agent } from '../setup/setup-agents';
import { exchangeContacts } from '../helpers/flows/exchange-contacts';

describe('block contact', () => {
	let agent1: Agent;
	let agent2: Agent;

	before(async () => {
		agent1 = await setupAgent('agent1');
		agent2 = await setupAgent('agent2');
		await agent1.createProfile('Alice');
		await agent2.createProfile('Bob');
		await exchangeContacts(agent1, agent2);
	});

	it('blocks and unblocks from chat settings, showing the indicator', async () => {
		// agent1 is already on the direct chat with Bob after exchangeContacts.
		await agent1.directChatPage.settingsLink.click();
		await agent1.chatSettingsPage.ready();

		await agent1.chatSettingsPage.blockToggle.click();
		await agent1.chatSettingsPage.blockConfirm.waitForDisplayed();
		await agent1.chatSettingsPage.blockConfirm.click();

		// Back on the chat, the blocked banner + name icon appear.
		await agent1.chatSettingsPage.back.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.blockedBanner.waitForDisplayed();
		await agent1.directChatPage.blockedNameIcon.waitForDisplayed();

		// The chat-list row shows the icon too.
		await agent1.directChatPage.back.click();
		await agent1.homePage.ready();
		await agent1.homePage.blockedRowIcon.waitForDisplayed();

		// Unblock from the chat's banner.
		await agent1.homePage.chatRow.click();
		await agent1.directChatPage.ready();
		await agent1.directChatPage.unblockButton.click();
		await agent1.directChatPage.blockConfirm.waitForDisplayed();
		await agent1.directChatPage.blockConfirm.click();
		await agent1.directChatPage.blockedBanner.waitForDisplayed({
			reverse: true,
		});
	});
});
```

Before writing, confirm the exact helper names by reading `e2e-tests/setup/setup-agents.ts` (profile creation helper) and `e2e-tests/helpers/pages/home-page.ts` (`ready()`, `chatRow`). Adjust `createProfile`/`ready` calls to match the real API — do not invent methods.

- [ ] **Step 5: Run the spec**

Run: `just test e2e block-contact`
Expected: PASS. (First run builds the Tauri binary; allow several minutes.)

- [ ] **Step 6: Commit**

```bash
git add e2e-tests/helpers/pages/direct-chats/direct-chat-page.ts e2e-tests/helpers/pages/direct-chats/chat-settings-page.ts e2e-tests/helpers/pages/home-page.ts e2e-tests/specs/block-contact.spec.ts
git commit -m "E2E coverage for block-contact UI"
```

---

### Task 7: Format + final verification

- [ ] **Step 1: Format**

Run: `just format`

- [ ] **Step 2: Full type-check**

Run (from `ui/`): `pnpm check` → clean.

- [ ] **Step 3: Manual polish pass (per CLAUDE.md UI requirement)**

Start the app (`start-dev` skill), and verify against the Signal reference (`signal-reference/android/direct-chat/13-block-user-confirmation.png`, `signal-reference/ios/direct-chat/03-unblock-confirmation-dialog.png`): dialog copy/layout, the ⊘ banner, and the ⊘ next to names in list + title in both light and dark, LTR and RTL. Kill dev processes when done.

- [ ] **Step 4: Commit any polish fixes**

```bash
git add -A && git commit -m "Polish block-contact UI"
```

## Notes for the executor

- `rejectContactRequest` and its dialog stay in `direct-chats/[agentId]/+page.svelte` intentionally — removing them is a separate follow-up.
- The ⊘ icon is scoped to direct chats only. Blocking a contact who is in a group is supported by the backend, but we do NOT render the icon in group contexts.
- If `pnpm check` reports missing `m.*` functions, the paraglide message module needs regenerating — build the UI package or restart the dev server; the source keys are in `en.json`.
