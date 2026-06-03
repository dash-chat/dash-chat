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
			role="button"
			tabindex={0}
			aria-label={isLocal
				? m.connectionStatusLocalAriaLabel()
				: m.connectionStatusDisconnectedAriaLabel()}
			onClick={() => (dialogOpened = true)}
			onkeydown={(e: KeyboardEvent) => {
				if (e.key === 'Enter' || e.key === ' ') {
					e.preventDefault();
					dialogOpened = true;
				}
			}}
		>
			{#if isLocal}
				<wa-icon
					class="connection-status"
					src="/localmailboxserver.svg"
					aria-hidden="true"
				></wa-icon>
			{:else}
				<wa-icon
					class="connection-status"
					src={wrapPathInSvg(mdiEmoticonPoop)}
					aria-hidden="true"
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
					aria-hidden="true"
				></wa-icon>
				<span data-testid="connection-status-dialog-title">
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
			<span data-testid="connection-status-dialog-description">
				{#if isLocal}
					{m.connectionStatusLocalDescription({ count: localCount })}
				{:else}
					{m.connectionStatusDisconnectedDescription()}
				{/if}
			</span>

			{#snippet buttons()}
				<DialogButton
					data-testid="connection-status-dialog-close"
					onClick={() => (dialogOpened = false)}
				>
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
	:global(.dark) .connection-status {
		color: white;
	}
</style>
