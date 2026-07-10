<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		fullName,
		type ChatId,
		type DeviceId,
		type Hash,
		type MailboxTrackerStore,
		type Message,
		type MessagesStore,
		type Profile,
	} from 'dash-chat-stores';
	import type { MessagePosition } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import ReplyQuote from './ReplyQuote.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import QuickReactionBar from './QuickReactionBar.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { m } from '$lib/paraglide/messages';
	import { longpress } from '$lib/actions/longpress';
	import { toggleReaction } from '$lib/utils/reactions';

	let {
		message,
		position,
		myDeviceId,
		searchQuery,
		chatId,
		sender,
		showSenderName = false,
		onShowHistory,
		showAvatar = false,
		canDelete = false,
		onDelete,
		onReply,
		replyAuthorName,
		onNavigateToMessage,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		sender: Profile | undefined;
		showSenderName?: boolean;
		onShowHistory?: () => void;
		showAvatar?: boolean;
		canDelete?: boolean;
		onDelete?: () => void;
		/** Start composing a reply to this message. */
		onReply?: () => void;
		/** Display name of the author quoted in this message's reply. */
		replyAuthorName?: string;
		/** Scroll the chat to the quoted message. */
		onNavigateToMessage?: (target: Hash) => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const senderDisplayName = $derived(
		sender && sender.name ? fullName(sender) : m.unknownSender(),
	);

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

{#snippet editedIndicator()}
	<EditedIndicator class="quiet" {onShowHistory} />
{/snippet}

{#snippet metadata()}
	<MessageTimestamp timestamp={message.timestamp} class="quiet" />
{/snippet}

<div
	bind:this={messageEl}
	use:longpress={{
		onLongPress: () => {
			if (!message.deleted) reactionsOpened = true;
		},
	}}
>
	<div class="row items-end gap-2">
		{#if showAvatar}
			{#if isLast}
				<Avatar
					image={sender?.avatar}
					initials={sender?.name.slice(0, 2)}
					size="2rem"
				/>
			{:else}
				<div class="shrink-0" style="width: 2rem"></div>
			{/if}
		{/if}
		<Card
			raised
			contentWrapPadding="p-2"
			class={`message others-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
		>
			{#if message.deleted}
				<div
					class="flex items-end gap-2.5 px-1 italic"
					data-testid="deleted-message"
				>
					{m.thisMessageWasDeleted()}
					{#if isLast}
						<MessageTimestamp timestamp={message.timestamp} class="quiet" />
					{/if}
				</div>
			{:else}
				{#if message.reply}
					<ReplyQuote
						reply={message.reply}
						authorName={replyAuthorName}
						onNavigate={onNavigateToMessage}
					/>
				{/if}
				<MessageContent
					{message}
					{searchQuery}
					senderName={senderDisplayName}
					{showSenderName}
					editedIndicator={message.editedAt ? editedIndicator : undefined}
					metadata={isLast ? metadata : undefined}
				/>
			{/if}
		</Card>
	</div>
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
	{canDelete}
	{onDelete}
	{onReply}
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
