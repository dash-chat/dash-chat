<script lang="ts">
	import { Card } from 'konsta/svelte';
	import type {
		ChatId,
		DeviceId,
		MailboxTrackerStore,
		Message,
	} from 'dash-chat-stores';
	import { highlightMatch, type MessagePosition } from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import Reactions from './Reactions.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';

	let {
		message,
		position,
		myDeviceId,
		searchQuery,
		onToggleReaction,
		chatId,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		onToggleReaction: (emoji: string) => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');

	const mailboxTrackerStore: MailboxTrackerStore = getContext(
		'mailbox-tracker-store',
	);

	const syncStatus = $derived(
		useReactiveValue(
			mailboxTrackerStore.syncStatusForOp,
			chatId,
			message.author,
			message.seqNum,
		),
	);
	const connectionStatus = useReactiveValue(
		mailboxTrackerStore.connectionStatus,
	);

	const isOfflineMessage = $derived(
		$syncStatus !== undefined &&
			$connectionStatus !== undefined &&
			!$syncStatus.syncedWithCloudMailbox &&
			!$connectionStatus.connectedToCloudMailboxServer,
	);
</script>

<Card
	raised
	contentWrapPadding="p-2"
	class={`message others-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
>
	<div class="row gap-2 mx-1" style="align-items: end">
		<span class="flex-1">
			{#if searchQuery}
				{@html highlightMatch(message.content, searchQuery)}
			{:else}
				{message.content}
			{/if}
		</span>

		{#if isLast}
			<MessageTimestamp timestamp={message.timestamp} class="quiet" />
		{/if}
	</div>
</Card>
{#if Object.keys(message.reactions).length}
	<div class="flex justify-end -mt-1.5 mb-0.5 px-1">
		<Reactions reactions={message.reactions} {myDeviceId} {onToggleReaction} />
	</div>
{/if}

<style>
	:global(.others-message) {
		align-self: start;
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	:global(.others-message.first-message) {
		border-end-start-radius: 4px;
	}
	:global(.others-message.middle-message) {
		border-start-start-radius: 4px;
		border-end-start-radius: 4px;
	}
	:global(.others-message.last-message) {
		border-start-start-radius: 4px;
	}

	:global(.others-message.offline-message) {
		border: 3px dashed rgb(255, 182, 193);
		background-clip: padding-box;
	}
	:global(.others-message.offline-message > div) {
		padding: calc(0.5rem - 2px) !important;
	}
</style>
