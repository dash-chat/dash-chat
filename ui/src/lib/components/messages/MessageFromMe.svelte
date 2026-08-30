<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		type ChatId,
		type DeviceId,
		type Hash,
		type MailboxTrackerStore,
		type Message,
		type MessagesStore,
		hasBody,
		isDeleted,
	} from 'dash-chat-stores';
	import { type MessagePosition } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import DeletedMessage from './DeletedMessage.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import MessageActionsOverlay from './MessageActionsOverlay.svelte';
	import MessageContextMenu from './MessageContextMenu.svelte';
	import MessageHoverToolbar from './MessageHoverToolbar.svelte';
	import SwipeToReply from './SwipeToReply.svelte';
	import MessageStatusIndicator from '$lib/components/messages/MessageStatusIndicator.svelte';
	import { isMobile } from '$lib/utils/environment';
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
		showDeliveryStatus = false,
		onEdit,
		onReply,
		onNavigateToMessage,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		/** Whether to render the message's delivery status indicator — see
		 * `endsDeliveryStatusRun`. */
		showDeliveryStatus?: boolean;
		onEdit?: () => void;
		onReply?: () => void;
		onNavigateToMessage?: (hash: Hash) => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const deleted = $derived(isDeleted(message.content));

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
	<span class="flex items-center gap-1 opacity-80">
		{#if editHistory.length > 0}
			<EditedIndicator />
		{/if}
		{#if isLast}
			<MessageTimestamp timestamp={message.timestamp} />
		{/if}
		{#if showDeliveryStatus && message.deliveryStatus}
			<MessageStatusIndicator status={message.deliveryStatus} />
		{/if}
	</span>
{/snippet}

{#snippet bubble()}
	<div bind:this={messageEl} class="relative max-w-[85%]">
		{#if !isMobile && hasBody(message.content)}
			<MessageHoverToolbar {message} {myDeviceId} {onEdit} {onReply} reverse />
		{/if}
		{#if deleted}
			<DeletedMessage {message} {position} {myDeviceId} />
		{:else}
			<Card
				raised
				contentWrapPadding="p-2"
				colors={{
					bgIos: 'bg-brand-primary',
					bgMaterial: 'bg-brand-primary',
					textIos: 'text-white',
					textMaterial: 'text-white',
				}}
				class={`message outgoing-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
			>
				<div class="flex flex-col gap-1">
					<MessageContent
						{message}
						{searchQuery}
						senderName={m.you()}
						mine
						{onNavigateToMessage}
						metadata={isLast || editHistory.length > 0 || showDeliveryStatus
							? metadata
							: undefined}
					/>
				</div>
			</Card>
		{/if}
		{#if Object.keys(reactions).length > 0}
			<div class="relative z-10 flex -mt-1.5 mb-0.5 px-1">
				<Reactions
					{reactions}
					onToggleReaction={emoji => toggleReaction(store, message, emoji)}
					onSheetOpen={() => (reactionsOpened = false)}
				/>
			</div>
		{/if}
	</div>
{/snippet}

{#snippet row()}
	<div class="group flex justify-end" use:longpress={{ onLongPress }}>
		{@render bubble()}
	</div>
{/snippet}

{#if deleted}
	<div class="group flex justify-end">{@render bubble()}</div>
{:else if isMobile}
	<SwipeToReply {onReply} target={messageEl}>{@render row()}</SwipeToReply>
{:else}
	{@render row()}
{/if}
{#if isMobile}
	<MessageActionsOverlay
		{message}
		{myDeviceId}
		{onEdit}
		{onReply}
		bind:opened={reactionsOpened}
		target={messageEl}
	/>
{:else}
	<MessageContextMenu
		{message}
		{myDeviceId}
		{onEdit}
		{onReply}
		bind:point={contextMenuPoint}
	/>
{/if}
