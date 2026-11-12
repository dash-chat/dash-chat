<script lang="ts">
	import type {
		ChatId,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { connectionStatus } from './connection-status';

	let { chatId }: { chatId: ChatId } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const inputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatusForAllMailboxes(),
		onlinePeersStore.onlinePeers(chatId),
	]);
</script>

{#await $inputs then [mailboxes, onlinePeers]}
	{#if connectionStatus(mailboxes, onlinePeers.length > 0, renderingResumedAt.value).status === 'p2p'}
		<span
			class="block truncate text-xs/4 font-normal text-[#525252] dark:text-white/70"
			data-testid="connected-peers-count"
		>
			{m.connectedPeersCount({ count: onlinePeers.length })}
		</span>
	{/if}
{:catch}
	<!-- A store that stalls rejects here, and can still resolve later. -->
{/await}
