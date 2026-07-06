<script lang="ts">
	import { Card } from 'konsta/svelte';
	import type {
		ChatId,
		DeviceId,
		MailboxTrackerStore,
		Message,
		MessagesStore,
	} from 'dash-chat-stores';
	import { highlightMatch, type MessagePosition } from './message-helpers';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import Reactions from './Reactions.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { longpress } from '$lib/actions/longpress';
	import { toggleReaction } from '$lib/utils/reactions';

	let {
		message,
		position,
		myDeviceId,
		searchQuery,
		chatId,
		sender,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		sender?: { name: string; color: string };
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');

	const store: MessagesStore = getContext('messages-store');

	let reactionsOpened = $state(false);
	let messageEl = $state<HTMLElement>();

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

<div
	bind:this={messageEl}
	use:longpress={{ onLongPress: () => (reactionsOpened = true) }}
>
	<Card
		raised
		contentWrapPadding="p-2"
		class={`message others-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
	>
		{#if sender}
			<div
				class="mx-1 mb-0.5 text-sm font-semibold text-start"
				style="color: {sender.color}"
				data-testid="group-message-sender-name"
			>
				{sender.name}
			</div>
		{/if}
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
	{#if Object.keys(message.reactions).length > 0}
		<div class="relative z-10 flex justify-end -mt-1.5 mb-0.5 px-1">
			<Reactions
				reactions={message.reactions}
				{myDeviceId}
				onToggleReaction={emoji =>
					toggleReaction(store, message, myDeviceId, emoji)}
			/>
		</div>
	{/if}
</div>
<QuickReactionBar
	{message}
	{myDeviceId}
	bind:opened={reactionsOpened}
	target={messageEl}
/>

<style>
	:global(.others-message) {
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
