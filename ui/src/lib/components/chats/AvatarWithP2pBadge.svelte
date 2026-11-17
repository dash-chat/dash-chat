<script lang="ts">
	import type {
		ChatSummary,
		IOnlinePeersStore,
		MailboxTrackerStore,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { renderingResumedAt } from '$lib/stores/rendering-resumed.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { peersConnectedOverP2p } from '../connection/connection-status';
	import P2pBadge from '../connection/P2pBadge.svelte';
	import GroupAvatar from '../profiles/GroupAvatar.svelte';
	import PeerAvatar from '../profiles/PeerAvatar.svelte';

	let { summary, size }: { summary: ChatSummary; size: number } = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const onlinePeersStore: IOnlinePeersStore = getContext('online-peers-store');

	const mailboxes = useReactiveValue(
		mailboxTrackerStore.connectionStatusForAllMailboxes,
	);
	const onlinePeers = $derived(
		useReactiveValue(onlinePeersStore.onlinePeers, summary.chatId),
	);
	const connectedOverP2p = $derived(
		$mailboxes !== undefined &&
			$onlinePeers !== undefined &&
			peersConnectedOverP2p($mailboxes, $onlinePeers, renderingResumedAt.value)
				.length > 0,
	);
</script>

<P2pBadge show={connectedOverP2p} testId="chat-row-p2p-badge">
	{#if summary.type === 'GroupChat'}
		<GroupAvatar image={summary.avatar} {size} />
	{:else}
		<PeerAvatar
			profile={summary.waitingForProfile ? undefined : summary}
			{size}
		/>
	{/if}
</P2pBadge>
