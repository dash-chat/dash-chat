<script lang="ts">
	import type { MailboxTrackerStore } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext, type Snippet } from 'svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiEmoticonPoop } from '@mdi/js';
	import { Chip, Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);

	const connectionStatus = useReactivePromise(
		mailboxTrackerStore.connectionStatus,
	);

	let dialogOpened = $state(false);

	// Konsta's Dialog type intersects with HTMLAttributes (which has `title?: string`),
	// so a Snippet for `title` can't be passed without a cast.
	const asTitle = (snippet: Snippet) => snippet as unknown as string;
</script>

{#await $connectionStatus then connectionStatus}
	{@const localCount = connectionStatus.connectedLocalMailboxCount}
	{@const isLocal = localCount > 0}
	{#if !connectionStatus.connectedToCloudMailboxServer}
		<Chip
			data-testid="connection-status"
			data-status={isLocal ? 'local' : 'disconnected'}
			class="p-1 cursor-pointer"
			colors={{
				fillBgIos: 'bg-black/10 dark:bg-brand-primary',
				fillBgMaterial: 'bg-md-light-secondary-container dark:bg-brand-primary',
			}}
			onClick={() => (dialogOpened = true)}
		>
			{#if isLocal}
				<wa-icon
					class="connection-status"
					src="/localmailboxserver.svg"
					aria-label="connected-to-local-mailbox"
				></wa-icon>
			{:else}
				<wa-icon
					class="connection-status"
					src={wrapPathInSvg(mdiEmoticonPoop)}
					aria-label="disconnected"
				></wa-icon>
			{/if}
		</Chip>

		{#snippet titleSlot()}
			<div class="flex flex-row items-center gap-2">
				<wa-icon
					class="connection-status-dialog-icon"
					src={isLocal
						? '/localmailboxserver.svg'
						: wrapPathInSvg(mdiEmoticonPoop)}
					aria-label={isLocal ? 'connected-to-local-mailbox' : 'disconnected'}
				></wa-icon>
				<span>
					{isLocal
						? m.connectionStatusLocalTitle()
						: m.connectionStatusDisconnectedTitle()}
				</span>
			</div>
		{/snippet}

		<Dialog
			data-testid="connection-status-dialog"
			opened={dialogOpened}
			onBackdropClick={() => (dialogOpened = false)}
			title={asTitle(titleSlot)}
		>
			<span>
				{#if isLocal}
					{m.connectionStatusLocalDescription({ count: localCount })}
				{:else}
					{m.connectionStatusDisconnectedDescription()}
				{/if}
			</span>

			{#snippet buttons()}
				<DialogButton onClick={() => (dialogOpened = false)}>
					{m.close()}
				</DialogButton>
			{/snippet}
		</Dialog>
	{/if}
{/await}

<style>
	.connection-status {
		color: var(--color-brand-primary);
		font-size: 28px;
	}
	.connection-status-dialog-icon {
		font-size: 32px;
	}
	:global(.dark) .connection-status,
	:global(.dark) .connection-status-dialog-icon {
		color: white;
	}
</style>
