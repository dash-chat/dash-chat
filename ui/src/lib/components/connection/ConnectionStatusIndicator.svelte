<script lang="ts">
	import type {
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { getContext, type Snippet } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import {
		connectionChipStatus,
		type ConnectionChipStatus,
	} from './connection-chip';
	import { mdiEmoticonPoop } from '@mdi/js';
	import { Chip, Dialog, DialogButton } from 'konsta/svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import cloudSvg from '$lib/assets/connection-status/cloud.svg?raw';
	import localMailboxServerSvg from '$lib/assets/connection-status/localmailboxserver.svg?raw';
	import p2pSvg from '$lib/assets/connection-status/p2p.svg?raw';

	let { chatId }: { chatId: ChatId } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const chipInputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatus(),
		onlinePeersStore.onlinePeers(chatId),
	]);

	const rawIcons: Record<
		Exclude<ConnectionChipStatus, 'disconnected'>,
		string
	> = {
		cloud: cloudSvg,
		pending: cloudSvg,
		local: localMailboxServerSvg,
		p2p: p2pSvg,
	};

	const ariaLabels: Record<ConnectionChipStatus, () => string> = {
		cloud: m.connectionStatusCloudAriaLabel,
		pending: m.connectionStatusPendingAriaLabel,
		local: m.connectionStatusLocalAriaLabel,
		p2p: m.connectionStatusP2pAriaLabel,
		disconnected: m.connectionStatusDisconnectedAriaLabel,
	};

	const titles: Record<ConnectionChipStatus, () => string> = {
		cloud: m.connectionStatusCloudTitle,
		pending: m.connectionStatusPendingTitle,
		local: m.connectionStatusLocalTitle,
		p2p: m.connectionStatusP2pTitle,
		disconnected: m.connectionStatusDisconnectedTitle,
	};

	const descriptions: Record<
		ConnectionChipStatus,
		(localCount: number) => string
	> = {
		cloud: () => m.connectionStatusCloudDescription(),
		pending: () => m.connectionStatusPendingDescription(),
		local: count => m.connectionStatusLocalDescription({ count }),
		p2p: () => m.connectionStatusP2pDescription(),
		disconnected: () => m.connectionStatusDisconnectedDescription(),
	};

	let dialogOpened = $state(false);

	// Konsta's Dialog type intersects with HTMLAttributes (which has `title?: string`),
	// so a Snippet for `title` can't be passed without a cast.
	const asTitle = (snippet: Snippet) => snippet as unknown as string;
</script>

{#snippet icon(status: ConnectionChipStatus)}
	{#if status === 'disconnected'}
		<svg viewBox="0 0 24 24" fill="currentColor"
			><path d={mdiEmoticonPoop} /></svg
		>
	{:else}
		{@html rawIcons[status]}
	{/if}
{/snippet}

{#await $chipInputs then [connectionStatus, onlinePeers]}
	{@const localCount = connectionStatus.connectedLocalMailboxCount}
	{@const status = connectionChipStatus(
		connectionStatus,
		onlinePeers.length > 0,
		renderingResumedAt.value,
	)}
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
		<span
			class="connection-status connection-status-{status} flex h-[1em] justify-center *:h-full *:w-auto"
			aria-hidden="true"
		>
			{@render icon(status)}
		</span>
	</Chip>

	{#snippet titleSlot()}
		<div class="flex flex-row items-center gap-2">
			<span
				class="connection-status-dialog-icon connection-status-dialog-icon-{status} flex h-[1em] justify-center *:h-full *:w-auto"
				aria-hidden="true"
			>
				{@render icon(status)}
			</span>
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
					{descriptions[status](localCount)}
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
{/await}

<style>
	.connection-status {
		color: var(--color-brand-primary);
		font-size: 28px;
		width: 35px;
	}
	.connection-status-local,
	.connection-status-p2p {
		font-size: 22px;
	}
	.connection-status-cloud,
	.connection-status-pending {
		font-size: 16px;
	}
	.connection-status-pending {
		animation: connection-pending 1.2s ease-in-out infinite;
	}
	@keyframes connection-pending {
		50% {
			opacity: 0.15;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.connection-status-pending {
			animation: none;
			opacity: 0.5;
		}
	}
	.connection-status-dialog-icon {
		font-size: 32px;
	}
	.connection-status-dialog-icon-local,
	.connection-status-dialog-icon-p2p {
		font-size: 26px;
	}
	.connection-status-dialog-icon-cloud,
	.connection-status-dialog-icon-pending {
		font-size: 20px;
	}

	:global(.dark) .connection-status {
		color: white;
	}
</style>
