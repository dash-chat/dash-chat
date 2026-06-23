<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		fullName,
		type ChatId,
		type DeviceId,
		type MailboxTrackerStore,
		type Message,
		type Profile,
	} from 'dash-chat-stores';
	import type { MessagePosition } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages';

	let {
		message,
		position,
		myDeviceId,
		searchQuery,
		onToggleReaction,
		chatId,
		sender,
		showSenderName = false,
		onShowHistory,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		onToggleReaction: (emoji: string) => void;
		sender: Profile | undefined;
		showSenderName?: boolean;
		onShowHistory?: () => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const senderDisplayName = $derived(
		sender && sender.name ? fullName(sender) : m.unknownSender(),
	);

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
	{#if message.editedAt}
		<EditedIndicator class="quiet" {onShowHistory} />
	{/if}
	<MessageTimestamp timestamp={message.timestamp} class="quiet" />
{/snippet}

<Card
	raised
	contentWrapPadding="p-2"
	class={`message others-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
>
	<MessageContent
		{message}
		{searchQuery}
		senderName={senderDisplayName}
		{showSenderName}
		metadata={isLast ? metadata : undefined}
	/>
</Card>
{#if Object.keys(message.reactions).length}
	<div class="flex justify-end -mt-1.5 mb-0.5 px-1">
		<Reactions reactions={message.reactions} {myDeviceId} {onToggleReaction} />
	</div>
{/if}

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
