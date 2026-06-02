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
	import MessageStatusIndicator from '$lib/components/messages/MessageStatusIndicator.svelte';
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
	class={`message my-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
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
			<div class="flex items-center gap-1">
				<MessageTimestamp timestamp={message.timestamp} class="dark-quiet" />

				<MessageStatusIndicator
					{chatId}
					author={message.author}
					seq={message.seqNum}
				/>
			</div>
		{/if}
	</div>
</Card>
{#if Object.keys(message.reactions).length}
	<div class="flex -mt-1.5 mb-0.5 px-1">
		<Reactions reactions={message.reactions} {myDeviceId} {onToggleReaction} />
	</div>
{/if}

<style>
	:global(.my-message) {
		align-self: end;
		background-color: var(--color-brand-primary);
		color: white;
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	:global(.my-message.first-message) {
		border-end-end-radius: 4px;
	}
	:global(.my-message.middle-message) {
		border-start-end-radius: 4px;
		border-end-end-radius: 4px;
	}
	:global(.my-message.last-message) {
		border-start-end-radius: 4px;
	}

	:global(.my-message.offline-message) {
		border: 3px dashed rgb(255, 182, 193);
		background-clip: padding-box;
	}
	:global(.my-message.offline-message > div) {
		padding: calc(0.5rem - 2px) !important;
	}
</style>
