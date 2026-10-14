<script lang="ts">
	import type {
		AgentId,
		ChatId,
		IConnectedPeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { getContext, type Snippet } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		connectionChipStatus,
		type ConnectionChipStatus,
	} from './connection-chip';
	import { mdiEmoticonPoop } from '@mdi/js';
	import { Chip, Dialog, DialogButton } from 'konsta/svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';

	let { chatId, peers }: { chatId: ChatId; peers: AgentId[] } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const connectedPeersStore: IConnectedPeersStore = getContext(
		'connected-peers-store',
	);

	const connectionStatus = useReactivePromise(
		mailboxTrackerStore.connectionStatus,
	);
	const connectedPeers = useReactiveValue(() =>
		connectedPeersStore.connectedPeers(chatId),
	);
	const connectedToPeer = $derived(
		($connectedPeers ?? []).some(peer => peers.includes(peer)),
	);

	const icons: Record<ConnectionChipStatus, string> = {
		local: '/localmailboxserver.svg',
		p2p: '/p2p.svg',
		disconnected: wrapPathInSvg(mdiEmoticonPoop),
	};
	const titles: Record<ConnectionChipStatus, () => string> = {
		local: m.connectionStatusLocalTitle,
		p2p: m.connectionStatusP2pTitle,
		disconnected: m.connectionStatusDisconnectedTitle,
	};
	const ariaLabels: Record<ConnectionChipStatus, () => string> = {
		local: m.connectionStatusLocalAriaLabel,
		p2p: m.connectionStatusP2pAriaLabel,
		disconnected: m.connectionStatusDisconnectedAriaLabel,
	};

	let dialogOpened = $state(false);

	// Konsta's Dialog type intersects with HTMLAttributes (which has `title?: string`),
	// so a Snippet for `title` can't be passed without a cast.
	const asTitle = (snippet: Snippet) => snippet as unknown as string;
</script>

{#await $connectionStatus then connectionStatus}
	{@const localCount = connectionStatus.connectedLocalMailboxCount}
	{@const status = connectionChipStatus(
		connectionStatus,
		connectedToPeer,
		renderingResumedAt.value,
	)}
	{#if status !== null}
		<Chip
			data-testid="connection-status"
			data-status={status}
			class="p-1 cursor-pointer"
			colors={{
				fillBgIos: 'bg-black/10 dark:bg-brand-primary',
				fillBgMaterial: 'bg-md-light-secondary-container dark:bg-brand-primary',
			}}
			role="button"
			tabindex={0}
			aria-label={ariaLabels[status]()}
			onClick={() => (dialogOpened = true)}
			onkeydown={(e: KeyboardEvent) => {
				if (e.key === 'Enter' || e.key === ' ') {
					e.preventDefault();
					dialogOpened = true;
				}
			}}
		>
			<wa-icon
				class="connection-status connection-status-{status}"
				src={icons[status]}
				aria-hidden="true"
			></wa-icon>
		</Chip>

		{#snippet titleSlot()}
			<div class="flex flex-row items-center gap-2">
				<wa-icon
					class="connection-status-dialog-icon"
					class:connection-status-dialog-icon-local={status === 'local'}
					src={icons[status]}
					aria-hidden="true"
				></wa-icon>
				<span data-testid="connection-status-dialog-title">
					{titles[status]()}
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
						{#if status === 'local'}
							{m.connectionStatusLocalDescription({ count: localCount })}
						{:else if status === 'p2p'}
							{m.connectionStatusP2pDescription()}
						{:else}
							{m.connectionStatusDisconnectedDescription()}
						{/if}
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
	{/if}
{/await}

<style>
	.connection-status {
		color: var(--color-brand-primary);
		font-size: 28px;
		width: 35px;
	}
	.connection-status-local {
		font-size: 22px;
	}
	.connection-status-dialog-icon {
		font-size: 32px;
	}
	.connection-status-dialog-icon-local {
		font-size: 26px;
	}

	:global(.dark) .connection-status {
		color: white;
	}
</style>
