<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type {
		AgentId,
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { mdiCheckBold } from '@mdi/js';
	import { Dialog, DialogButton, useTheme } from 'konsta/svelte';
	import { reactive, relay } from 'signalium';
	import { getContext, type Snippet } from 'svelte';
	import { fly } from 'svelte/transition';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		connectionStatus,
		internetStatus,
		reachableLocalMailboxCount,
		type ConnectionStatus,
	} from './connection-status';
	import type { ConnectionStatusTag } from './connection-status-tags';
	import ConnectionStatusIcon from './ConnectionStatusIcon.svelte';

	/** Takes no space in its parent, so the page decides how its content makes
	 * room: pair it with `connectionBarNavbarClass` on the page's `Navbar`. */
	let { chatId, opened }: { chatId?: ChatId; opened: boolean } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const noPeers = reactive(() =>
		relay<AgentId[]>(state => {
			state.value = [];
		}),
	);

	const inputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatusForAllMailboxes(),
		chatId === undefined ? noPeers() : onlinePeersStore.onlinePeers(chatId),
	]);

	const theme = $derived(useTheme());

	const titles: Record<ConnectionStatusTag, () => string> = {
		cloud: m.connectionStatusCloudTitle,
		pending: m.connectionStatusPendingTitle,
		local: m.connectionStatusLocalTitle,
		p2p: m.connectionStatusP2pTitle,
		disconnected: m.connectionStatusDisconnectedTitle,
	};

	function description(status: ConnectionStatus): string {
		switch (status.status) {
			case 'cloud':
				return m.connectionStatusCloudDescription();
			case 'pending':
				return m.connectionStatusPendingDescription();
			case 'local':
				return m.connectionStatusLocalDescription({ count: status.count });
			case 'p2p':
				return m.connectionStatusP2pDescription();
			case 'disconnected':
				return m.connectionStatusDisconnectedDescription();
		}
	}

	let dialogOpened = $state(false);

	// Konsta's Dialog type intersects with HTMLAttributes (which has `title?: string`),
	// so a Snippet for `title` can't be passed without a cast.
	const asTitle = (snippet: Snippet) => snippet as unknown as string;
</script>

{#snippet channel(
	status: ConnectionStatusTag,
	connected: boolean,
	testId: string,
)}
	<span class="relative" data-testid={testId} data-connected={connected}>
		<ConnectionStatusIcon {status} />
		{#if connected}
			<wa-icon
				class="absolute -end-3 -top-1.5 text-xs text-[#34C759]"
				src={wrapPathInSvg(mdiCheckBold)}
			></wa-icon>
		{/if}
	</span>
{/snippet}

{#snippet optionalChannel(
	status: ConnectionStatusTag,
	connected: boolean,
	testId: string,
)}
	<span class={connected ? '' : 'opacity-40'}>
		{@render channel(status, connected, testId)}
	</span>
{/snippet}

{#snippet content(
	status: ConnectionStatus,
	internet: ConnectionStatusTag,
	hubCount: number,
	peerOnline: boolean,
)}
	<button
		type="button"
		data-testid="connection-status-bar"
		class="flex h-14 w-full cursor-pointer items-center gap-6 px-4"
		onclick={() => (dialogOpened = true)}
	>
		<span class="flex shrink-0 items-center gap-6" aria-hidden="true">
			{@render channel(
				internet,
				internet === 'cloud',
				'connection-status-bar-internet',
			)}
			{@render optionalChannel(
				'local',
				hubCount > 0,
				'connection-status-bar-hub',
			)}
			{#if chatId !== undefined}
				{@render optionalChannel(
					'p2p',
					peerOnline,
					'connection-status-bar-p2p',
				)}
			{/if}
		</span>
		<span
			class="ms-auto min-w-0 truncate text-sm text-[#525252] dark:text-white/70"
		>
			{titles[status.status]()}
		</span>
	</button>

	{#snippet titleSlot()}
		<div class="flex flex-row items-center gap-3">
			<span aria-hidden="true">
				<ConnectionStatusIcon status={status.status} />
			</span>
			<span data-testid="connection-status-dialog-title">
				{titles[status.status]()}
			</span>
		</div>
	{/snippet}

	<Modal bind:opened={dialogOpened}>
		{#snippet children(modal)}
			<Dialog
				data-testid="connection-status-dialog"
				opened={modal.opened}
				onBackdropClick={modal.close}
				title={asTitle(titleSlot)}
			>
				<span data-testid="connection-status-dialog-description">
					{description(status)}
				</span>

				{#snippet buttons()}
					<DialogButton
						data-testid="connection-status-dialog-close"
						onClick={modal.close}
					>
						{m.close()}
					</DialogButton>
				{/snippet}
			</Dialog>
		{/snippet}
	</Modal>
{/snippet}

<div class="sticky top-0 z-30 h-0">
	{#if opened}
		<div
			class="pt-safe {theme === 'ios'
				? 'bg-ios-light-surface dark:bg-ios-dark-surface'
				: 'bg-md-light-surface dark:bg-md-dark-surface'}"
			transition:fly={{ y: '-100%', opacity: 1, duration: 200 }}
		>
			{#await $inputs}
				{@render content({ status: 'pending' }, 'pending', 0, false)}
			{:then [mailboxes, onlinePeers]}
				{@render content(
					connectionStatus(
						mailboxes,
						onlinePeers.length > 0,
						renderingResumedAt.value,
					),
					internetStatus(mailboxes, renderingResumedAt.value),
					reachableLocalMailboxCount(mailboxes),
					onlinePeers.length > 0,
				)}
			{:catch}
				<!-- A store that stalls rejects here, and can still resolve later. -->
				{@render content({ status: 'pending' }, 'pending', 0, false)}
			{/await}
		</div>
	{/if}
</div>
