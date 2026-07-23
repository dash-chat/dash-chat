<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		type ChatId,
		type DeviceId,
		type MailboxTrackerStore,
		type Message,
		type MessagesStore,
		hasBody,
	} from 'dash-chat-stores';
	import {
		canDeleteMessageForEveryone,
		canEditMessage,
		type MessagePosition,
	} from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import MessageActions from './MessageActions.svelte';
	import MessageHoverToolbar from './MessageHoverToolbar.svelte';
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
		onEdit,
		onDelete,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		onEdit?: () => void;
		onDelete?: () => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const canEdit = $derived(canEditMessage(message, myDeviceId));
	const canDelete = $derived(canDeleteMessageForEveryone(message, myDeviceId));

	const reactions = $derived(
		hasBody(message.content) ? message.content.reactions : {},
	);
	const editHistory = $derived(
		hasBody(message.content) ? message.content.editHistory : [],
	);

	const store: MessagesStore = getContext('messages-store');

	let reactionsOpened = $state(false);
	let messageEl = $state<HTMLElement>();
	let desktopOpen = $state<'reactions' | 'menu' | null>(null);
	let desktopAnchor = $state<HTMLElement | { x: number; y: number }>();

	function onLongPress(e: MouseEvent | TouchEvent) {
		if (hasBody(message.content)) {
			if (isMobile) {
				reactionsOpened = true;
			} else if (e instanceof MouseEvent) {
				desktopAnchor = { x: e.clientX, y: e.clientY };
				desktopOpen = 'menu';
			}
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

{#snippet editedIndicator()}
	<EditedIndicator class="dark-quiet" />
{/snippet}

{#snippet metadata()}
	<MessageTimestamp timestamp={message.timestamp} class="dark-quiet" />

	<MessageStatusIndicator
		{chatId}
		author={message.author}
		seq={message.seqNum}
	/>
{/snippet}

<div class="group flex justify-end" use:longpress={{ onLongPress }}>
	<div bind:this={messageEl} class="relative max-w-[85%]">
		{#if !isMobile}
			<div
				class="absolute end-full inset-y-0 me-1 flex items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 {desktopOpen !==
				null
					? '!opacity-100'
					: ''}"
			>
				<MessageHoverToolbar
					reverse
					onReact={el => {
						desktopAnchor = el;
						desktopOpen = 'reactions';
					}}
					onMenu={el => {
						desktopAnchor = el;
						desktopOpen = 'menu';
					}}
				/>
			</div>
		{/if}
		<Card
			raised
			contentWrapPadding="p-2"
			class={`message my-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
		>
			<MessageContent
				{message}
				{searchQuery}
				senderName={m.you()}
				deletedText={m.youDeletedThisMessage()}
				editedIndicator={editHistory.length > 0 ? editedIndicator : undefined}
				metadata={isLast ? metadata : undefined}
			/>
		</Card>
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
<MessageActions
	{message}
	{myDeviceId}
	{canEdit}
	{onEdit}
	{canDelete}
	{onDelete}
	bind:opened={reactionsOpened}
	bind:desktopOpen
	{desktopAnchor}
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
