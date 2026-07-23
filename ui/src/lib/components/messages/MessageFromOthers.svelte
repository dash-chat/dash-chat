<script lang="ts">
	import { Card } from 'konsta/svelte';
	import {
		fullName,
		hasBody,
		type ChatId,
		type DeviceId,
		type MailboxTrackerStore,
		type Message,
		type MessagesStore,
		type Profile,
	} from 'dash-chat-stores';
	import {
		canDeleteMessageForEveryone,
		type MessagePosition,
	} from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import MessageActions from './MessageActions.svelte';
	import MessageHoverToolbar from './MessageHoverToolbar.svelte';
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
		onDelete,
	}: {
		message: Message;
		position: MessagePosition;
		myDeviceId: DeviceId;
		chatId: ChatId;
		searchQuery: string;
		sender: Profile | undefined;
		showSenderName?: boolean;
		showAvatar?: boolean;
		onDelete?: () => void;
	} = $props();

	const isLast = $derived(position === 'last' || position === 'single');
	const senderDisplayName = $derived(
		sender && sender.name ? fullName(sender) : m.unknownSender(),
	);
	const canDeleteForEveryone = $derived(
		canDeleteMessageForEveryone(message, myDeviceId),
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
	<EditedIndicator class="quiet" />
{/snippet}

{#snippet metadata()}
	<MessageTimestamp timestamp={message.timestamp} class="quiet" />
{/snippet}

<div class="group flex justify-start" use:longpress={{ onLongPress }}>
	<div bind:this={messageEl} class="relative max-w-[85%]">
		{#if !isMobile}
			<div
				class="absolute start-full inset-y-0 ms-1 flex items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 {desktopOpen !==
				null
					? '!opacity-100'
					: ''}"
			>
				<MessageHoverToolbar
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
				<MessageContent
					{message}
					{searchQuery}
					senderName={senderDisplayName}
					{showSenderName}
					deletedText={m.thisMessageWasDeleted()}
					editedIndicator={editHistory.length > 0 ? editedIndicator : undefined}
					metadata={isLast ? metadata : undefined}
				/>
			</Card>
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
</div>
<MessageActions
	{message}
	{myDeviceId}
	{canDeleteForEveryone}
	{onDelete}
	bind:opened={reactionsOpened}
	bind:desktopOpen
	{desktopAnchor}
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
