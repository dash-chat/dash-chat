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
	import { type MessagePosition, hasMessageActions } from './message-helpers';
	import MessageContent from './MessageContent.svelte';
	import DeletedMessage from './DeletedMessage.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';
	import EditedIndicator from './EditedIndicator.svelte';
	import Reactions from './Reactions.svelte';
	import MessageActionsOverlay from './MessageActionsOverlay.svelte';
	import MessageContextMenu from './MessageContextMenu.svelte';
	import MessageHoverToolbar from './MessageHoverToolbar.svelte';
	import SwipeToReply from './SwipeToReply.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { isMobile } from '$lib/utils/environment';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { connectedToCloudMailboxServer } from '$lib/components/connection/connection-status';
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
		readOnly,
		onReply,
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
		readOnly: boolean;
		onReply?: () => void;
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
	// Most of each row's DOM, so only mounted once the bubble is pressed; a
	// finger resting on it mounts it early, so the long-press opens it animated.
	let actionsMounted = $state(false);
	let messageEl = $state<HTMLElement>();
	let slideEl = $state<HTMLElement>();
	let contextMenuPoint = $state<{ x: number; y: number }>();

	const hasActions = $derived(
		hasMessageActions(message, myDeviceId, readOnly, onReply !== undefined),
	);

	function onLongPress(e: MouseEvent | TouchEvent) {
		actionsMounted = true;
		if (isMobile) {
			// A writable chat's overlay always has the reaction bar to show.
			reactionsOpened = hasActions || !readOnly;
		} else if (hasActions && e instanceof MouseEvent) {
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
			message.logId,
			message.seqNum,
		),
	);
	const mailboxes = useReactiveValue(
		mailboxTrackerStore.connectionStatusForAllMailboxes,
	);

	const isOfflineMessage = $derived(
		$syncStatus !== undefined &&
			$mailboxes !== undefined &&
			!$syncStatus.syncedWithCloudMailbox &&
			!connectedToCloudMailboxServer($mailboxes),
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
	<div
		bind:this={messageEl}
		class={[
			'relative',
			isMobile && !isWideScreen.value
				? 'max-w-[calc(100%-2rem)]'
				: 'max-w-[85%]',
		]}
	>
		{#if !isMobile && hasBody(message.content)}
			<MessageHoverToolbar {message} {myDeviceId} {onReply} />
		{/if}
		<div class="row items-end gap-2">
			{#if showAvatar}
				{#if isLast}
					<Avatar
						image={sender?.avatar}
						initials={sender?.name.slice(0, 2)}
						size="1.75rem"
					/>
				{:else}
					<div class="shrink-0" style="width: 1.75rem"></div>
				{/if}
			{/if}
			<div bind:this={slideEl} class="column min-w-0">
				{#if deleted}
					<DeletedMessage
						{message}
						{position}
						{myDeviceId}
						senderName={senderDisplayName}
					/>
				{:else}
					<Card
						contentWrapPadding="px-2 py-(--message-padding-y)"
						class={`message incoming-message ${position}-message ${isOfflineMessage ? 'offline-message' : ''}`}
					>
						<MessageContent
							{message}
							{searchQuery}
							senderName={senderDisplayName}
							{showSenderName}
							{onNavigateToMessage}
							metadata={isLast || editHistory.length > 0 ? metadata : undefined}
						/>
					</Card>
				{/if}
				{#if Object.keys(reactions).length > 0}
					<div class="relative z-10 flex justify-end -mt-1.5 mb-0.5 px-1">
						<Reactions
							{reactions}
							onToggleReaction={emoji => toggleReaction(store, message, emoji)}
							onSheetOpen={() => (reactionsOpened = false)}
						/>
					</div>
				{/if}
			</div>
		</div>
	</div>
{/snippet}

{#snippet row()}
	<div
		class="group flex justify-start"
		use:longpress={{
			onLongPress,
			onPressStart: () => (actionsMounted = true),
		}}
	>
		{@render bubble()}
	</div>
{/snippet}

{#if deleted}
	<div class="group flex justify-start">{@render bubble()}</div>
{:else if isMobile}
	<SwipeToReply {readOnly} {onReply} target={slideEl}
		>{@render row()}</SwipeToReply
	>
{:else}
	{@render row()}
{/if}
{#if actionsMounted}
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
{/if}
