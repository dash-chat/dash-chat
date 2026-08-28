<script lang="ts">
	import { getContext } from 'svelte';
	import {
		type ChatId,
		type DeviceId,
		type MailboxTrackerStore,
		type MessageAckStore,
	} from 'dash-chat-stores';

	import { useReactivePromises } from '$lib/stores/use-signal';

	import StatusDeliveredIcon from './StatusDeliveredIcon.svelte';
	import StatusMailboxIcon from './StatusMailboxIcon.svelte';
	import StatusSendingIcon from './StatusSendingIcon.svelte';

	interface Props {
		chatId: ChatId;
		author: DeviceId;
		seq: number;
	}

	const props: Props = $props();

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);
	const messageAckStore: MessageAckStore = getContext('message-acks-store');

	const state = useReactivePromises(() => [
		mailboxTrackerStore.syncStatusForOp(props.chatId, props.author, props.seq),
		messageAckStore.acks(props.chatId),
	]);
</script>

{#await $state then [syncStatus, acks]}
	{@const acked = acks[props.author]}
	{@const status =
		acked !== undefined && acked.seq >= props.seq
			? 'delivered'
			: syncStatus.syncedWithCloudMailbox ||
				  syncStatus.syncedWithAnyLocalMailbox
				? 'mailbox'
				: 'sending'}
	<div
		data-testid="message-status"
		data-status={status}
		class="message-status"
		aria-label={status}
	>
		{#if status === 'delivered'}
			<StatusDeliveredIcon />
		{:else if status === 'mailbox'}
			<StatusMailboxIcon />
		{:else}
			<StatusSendingIcon />
		{/if}
	</div>
{/await}

<style>
	.message-status {
		opacity: 0.7;
		width: 0.875rem;
		height: 0.875rem;
	}
</style>
