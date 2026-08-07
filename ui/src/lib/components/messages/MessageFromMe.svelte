<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		type ChatId,
		type DeviceId,
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
	import MessageStatusIndicator from '$lib/components/messages/MessageStatusIndicator.svelte';
	import DeleteMessageDialog from './DeleteMessageDialog.svelte';
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
		onEdit,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		onEdit?: () => void;
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
	let confirmingDelete = $state(false);

	function onLongPress(e: MouseEvent | TouchEvent) {
		if (!hasBody(message.content)) return;
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
		<EditedIndicator class="dark-quiet" />
	{/if}
	{#if isLast}
		<MessageTimestamp timestamp={message.timestamp} class="dark-quiet" />

		<MessageStatusIndicator
			{chatId}
			author={message.author}
			seq={message.seqNum}
		/>
	{/if}
{/snippet}

<div class="group flex justify-end" use:longpress={{ onLongPress }}>
	<div bind:this={messageEl} class="relative max-w-[85%]">
		{#if !isMobile && hasBody(message.content)}
			<MessageHoverToolbar
				{message}
				{myDeviceId}
				{onEdit}
				onDelete={() => (confirmingDelete = true)}
				reverse
			/>
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
				<MessageContent
					{message}
					{searchQuery}
					senderName={m.you()}
					metadata={isLast || editHistory.length > 0 ? metadata : undefined}
				/>
			</Card>
		{/if}
		{#if Object.keys(reactions).length > 0}
			<div class="relative z-10 flex -mt-1.5 mb-0.5 px-1">
				<Reactions
					{reactions}
					{myDeviceId}
					onToggleReaction={emoji =>
						toggleReaction(store, message, myDeviceId, emoji)}
				/>
			</div>
		{/if}
	</div>
</div>
{#if isMobile}
	<MessageActionsOverlay
		{message}
		{myDeviceId}
		{onEdit}
		onDelete={() => (confirmingDelete = true)}
		bind:opened={reactionsOpened}
		target={messageEl}
	/>
{:else}
	<MessageContextMenu
		{message}
		{myDeviceId}
		{onEdit}
		onDelete={() => (confirmingDelete = true)}
		bind:point={contextMenuPoint}
	/>
{/if}

<DeleteMessageDialog {message} {myDeviceId} bind:opened={confirmingDelete} />
