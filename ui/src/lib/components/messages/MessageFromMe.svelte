<script lang="ts">
	import { Card } from 'konsta/svelte';
	import type {
		ChatId,
		DeviceId,
		MailboxTrackerStore,
		Message,
		MessagesStore,
	} from 'dash-chat-stores';
	import type { MessagePosition } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import Reactions from './Reactions.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import MessageStatusIndicator from '$lib/components/messages/MessageStatusIndicator.svelte';
	import { m } from '$lib/paraglide/messages.js';
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
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
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

{#snippet metadata()}
	<MessageTimestamp timestamp={message.timestamp} class="dark-quiet" />

	<MessageStatusIndicator
		{chatId}
		author={message.author}
		seq={message.seqNum}
	/>
{/snippet}

<div
	bind:this={messageEl}
	use:longpress={{ onLongPress: () => (reactionsOpened = true) }}
>
	<Card
		raised
		contentWrapPadding="p-2"
		class={`message my-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
	>
		<MessageContent
			{message}
			{searchQuery}
			senderName={m.you()}
			metadata={isLast ? metadata : undefined}
		/>
	</Card>
	{#if Object.keys(message.reactions).length > 0}
		<div class="flex -mt-1.5 mb-0.5 px-1">
			<Reactions
				reactions={message.reactions}
				{myDeviceId}
				onToggleReaction={emoji =>
					toggleReaction(store, message, myDeviceId, emoji)}
				onSheetOpen={() => (reactionsOpened = false)}
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
