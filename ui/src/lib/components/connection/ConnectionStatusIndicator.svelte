<script lang="ts">
	import type { MailboxTrackerStore } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiEmoticonPoop } from '@mdi/js';
	import { Chip } from 'konsta/svelte';

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);

	const connectionStatus = useReactivePromise(
		mailboxTrackerStore.connectionStatus,
	);
</script>

{#await $connectionStatus then connectionStatus}
	{#if !connectionStatus.connectedToCloudMailboxServer}
		<Chip
			data-testid="connection-status"
			data-status={connectionStatus.connectedToAnyLocalMailbox
				? 'local'
				: 'disconnected'}
			class="p-1"
			colors={{
				fillBgIos: 'bg-black/10 dark:bg-brand-primary',
				fillBgMaterial: 'bg-md-light-secondary-container dark:bg-brand-primary',
			}}
		>
			{#if connectionStatus.connectedToAnyLocalMailbox}
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
	{/if}
{/await}

<style>
	.connection-status {
		color: var(--color-brand-primary);
	}
	:global(.dark) .connection-status {
		color: white;
	}
</style>
