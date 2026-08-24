<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		fullName,
		hasBody,
		isDeleted,
		type ChatId,
		type DeviceId,
		type MailboxTrackerStore,
		type Hash,
		type Message,
		type MessagesStore,
		type Profile,
	} from 'dash-chat-stores';
	import type { MessagePosition } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import DeletedMessage from './DeletedMessage.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import MessageActionsOverlay from './MessageActionsOverlay.svelte';
	import MessageContextMenu from './MessageContextMenu.svelte';
	import MessageHoverToolbar from './MessageHoverToolbar.svelte';
	import ReplyQuote from './ReplyQuote.svelte';
	import SwipeToReply from './SwipeToReply.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { isMobile } from '$lib/utils/environment';
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
		showAvatar = false,
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
		showAvatar?: boolean;
		onReply?: () => void;
		/** Display name of the author quoted by this message's reply. */
		replyAuthorName?: string;
		onNavigateToMessage?: (hash: Hash) => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const deleted = $derived(isDeleted(message.content));
	const senderDisplayName = $derived(
		sender && sender.name ? fullName(sender) : m.unknownSender(),
	);

	const reactions = $derived(
		hasBody(message.content) ? message.content.reactions : {},
	);
	const editHistory = $derived(
		hasBody(message.content) ? message.content.editHistory : [],
	);

	const store: MessagesStore = getContext('messages-store');

	let reactionsOpened = $state(false);
	let messageEl = $state<HTMLElement>();
	let contextMenuPoint = $state<{ x: number; y: number }>();

	function onLongPress(e: MouseEvent | TouchEvent) {
		if (isMobile) {
			reactionsOpened = true;
		} else if (e instanceof MouseEvent) {
			contextMenuPoint = { x: e.clientX, y: e.clientY };
		}
	}

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
	{#if editHistory.length > 0}
		<EditedIndicator class="quiet" />
	{/if}
	{#if isLast}
		<MessageTimestamp timestamp={message.timestamp} class="quiet" />
	{/if}
{/snippet}

{#snippet bubble()}
	<div bind:this={messageEl} class="relative max-w-[85%]">
		{#if !isMobile && hasBody(message.content)}
			<MessageHoverToolbar {message} {myDeviceId} {onReply} />
		{/if}
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
			{#if deleted}
				<DeletedMessage
					{message}
					{position}
					{myDeviceId}
					senderName={senderDisplayName}
				/>
			{:else}
				<Card
					raised
					contentWrapPadding="p-2"
					class={`message incoming-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
				>
					<div class="flex flex-col gap-1">
						{#if message.replyQuote}
							<ReplyQuote
								reply={message.replyQuote}
								authorName={replyAuthorName}
								{myDeviceId}
								onNavigate={onNavigateToMessage}
							/>
						{/if}
						<MessageContent
							{message}
							{searchQuery}
							senderName={senderDisplayName}
							{showSenderName}
							metadata={isLast || editHistory.length > 0 ? metadata : undefined}
						/>
					</div>
				</Card>
			{/if}
		</div>
		{#if Object.keys(reactions).length > 0}
			<div class="relative z-10 flex justify-end -mt-1.5 mb-0.5 px-1">
				<Reactions
					{reactions}
					{myDeviceId}
					onToggleReaction={emoji =>
						toggleReaction(store, message, myDeviceId, emoji)}
				/>
			</div>
		{/if}
	</div>
{/snippet}

{#snippet row()}
	<div class="group flex justify-start" use:longpress={{ onLongPress }}>
		{@render bubble()}
	</div>
{/snippet}

{#if deleted}
	<div class="group flex justify-start">{@render bubble()}</div>
{:else if isMobile}
	<SwipeToReply {onReply}>{@render row()}</SwipeToReply>
{:else}
	{@render row()}
{/if}
{#if isMobile}
	<MessageActionsOverlay
		{message}
		{myDeviceId}
		{onReply}
		bind:opened={reactionsOpened}
		target={messageEl}
	/>
{:else}
	<MessageContextMenu
		{message}
		{myDeviceId}
		{onReply}
		bind:point={contextMenuPoint}
	/>
{/if}
