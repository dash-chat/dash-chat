<script lang="ts">
	import type {
		ChatSummary,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { useReactivePromises } from '$lib/stores/use-signal';
	import { connectionStatus } from '../connection/connection-status';
	import Avatar from '../profiles/Avatar.svelte';

	let { summary }: { summary: ChatSummary } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const inputs = useReactivePromises(() => [
		mailboxTrackerStore.connectionStatusForAllMailboxes(),
		onlinePeersStore.onlinePeers(summary.chatId),
	]);
</script>

<span class="avatar-with-p2p-badge relative inline-flex">
	<span class="avatar inline-flex">
		<Avatar
			image={summary.avatar}
			initials={summary.name.slice(0, 2)}
			waitingForProfile={summary.waitingForProfile}
		/>
	</span>
	{#await $inputs then [mailboxes, onlinePeers]}
		{#if connectionStatus(mailboxes, onlinePeers.length > 0, renderingResumedAt.value).status === 'p2p'}
			<span
				class="badge absolute end-0 bottom-0 size-[9px] rounded-full bg-[#22C55E]"
				role="img"
				aria-label={m.chatRowP2pBadgeLabel()}
				data-testid="chat-row-p2p-badge"
			></span>
		{/if}
	{:catch}
		<!-- A store that stalls rejects here, and can still resolve later. -->
	{/await}
</span>

<style>
	/* The ring is cut out of the avatar rather than painted, because the row
	   behind it changes colour: page, desktop sidebar, selected row. */
	.avatar-with-p2p-badge:has(.badge) .avatar {
		mask-image: radial-gradient(
			circle at calc(100% - 4.5px) calc(100% - 4.5px),
			transparent 6.75px,
			black 7.25px
		);
	}
	:global([dir='rtl']) .avatar-with-p2p-badge:has(.badge) .avatar {
		mask-image: radial-gradient(
			circle at 4.5px calc(100% - 4.5px),
			transparent 6.75px,
			black 7.25px
		);
	}
</style>
