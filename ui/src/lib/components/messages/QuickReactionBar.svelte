<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { QUICK_EMOJIS } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
<<<<<<< HEAD
	import { mdiDotsHorizontal, mdiPencil, mdiTrashCanOutline } from '@mdi/js';
	import { Popover, Sheet, Block, Chip } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
=======
	import { mdiDotsHorizontal } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import type { Message, DeviceId } from 'dash-chat-stores';
>>>>>>> edit-message-frontend
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
<<<<<<< HEAD
		/** Whether to offer an edit action (author, within the edit window). */
		canEdit?: boolean;
		onEdit?: () => void;
		/** Whether to offer a delete action (author, within the delete window). */
		canDelete?: boolean;
		onDelete?: () => void;
	}

	let {
		message,
		opened = $bindable(),
		target,
		myDeviceId,
		canEdit = false,
		onEdit,
		canDelete = false,
		onDelete,
	}: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let expanded = $state(false);

	// Reset the picker state once the reaction UI is closed.
	$effect(() => {
		if (!opened) expanded = false;
	});

	// Spotlight the focused message: raise it above the dimming backdrop (z-40),
	// while the popover card sits above it (z-50). Matches Signal's focused-message
	// lift.
	$effect(() => {
		if (!opened || !target) return;
		target.style.position = 'relative';
		target.style.zIndex = '45';
		return () => {
			target.style.position = '';
			target.style.zIndex = '';
		};
	});

	function close() {
		opened = false;
	}
=======
		onReact: (emoji: string) => void;
		/** Open the full emoji picker. */
		onExpand: () => void;
	}

	let { message, myDeviceId, onReact, onExpand }: Props = $props();
>>>>>>> edit-message-frontend

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}
<<<<<<< HEAD

	function react(emoji: string) {
		toggleReaction(store, message, myDeviceId, emoji);
		close();
	}

	function edit() {
		close();
		onEdit?.();
	}

	function del() {
		close();
		onDelete?.();
	}

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
=======
>>>>>>> edit-message-frontend
</script>

<div
	class="flex items-center gap-1 px-1 py-0.5"
	role="group"
	aria-label={m.quickReactions()}
	data-testid="quick-reaction-bar"
>
	{#each QUICK_EMOJIS as emoji}
		<button
			class="flex h-9 w-9 items-center justify-center rounded-full text-xl transition-transform hover:scale-110 {hasReacted(
				emoji,
			)
				? 'bg-blue-100 dark:bg-blue-900'
				: ''}"
			onclick={() => onReact(emoji)}
			data-testid={`quick-reaction-${emoji}`}
		>
			{emoji}
		</button>
	{/each}
	<IconButton
		onClick={onExpand}
		label={m.moreReactions()}
		testid="quick-reaction-more"
		class="!h-9 !w-9"
	>
<<<<<<< HEAD
		{#each QUICK_EMOJIS as emoji}
			<button
				class="flex h-9 w-9 items-center justify-center rounded-full text-xl transition-transform hover:scale-110 {hasReacted(
					emoji,
				)
					? 'bg-blue-100 dark:bg-blue-900'
					: ''}"
				onclick={() => react(emoji)}
				data-testid={`quick-reaction-${emoji}`}
			>
				{emoji}
			</button>
		{/each}
		{#if canEdit}
			<IconButton
				icon={mdiPencil}
				onClick={edit}
				label={m.edit()}
				testid="quick-edit-button"
				iconClass="text-xl"
			/>
		{/if}
		{#if canDelete}
			<IconButton
				icon={mdiTrashCanOutline}
				onClick={del}
				label={m.delete()}
				testid="quick-delete-button"
				iconClass="text-xl"
			/>
		{/if}
		<IconButton
			icon={mdiDotsHorizontal}
			onClick={() => (expanded = true)}
			label={m.moreReactions()}
			testid="quick-reaction-more"
			iconClass="text-xl"
		/>
	</div>
</Popover>

{#if opened}
	<Sheet class="pb-safe text-lg" opened={expanded} backdrop={false}>
		<div class="flex flex-col items-center">
			<SheetHandle />
		</div>
		{#if condensed.length > 0}
			<Block>
				{#each condensed as reaction}
					<button class="me-2 text-lg" onclick={() => react(reaction.emoji)}>
						<Chip class="border !border-white dark:!border-black">
							{reaction.emoji}{#if reaction.count > 1}<span class="ms-1"
									>{reaction.count}</span
								>{/if}
						</Chip>
					</button>
				{/each}
			</Block>
		{/if}
		<Block>
			<EmojiPickerWrapper onEmojiSelected={react}></EmojiPickerWrapper>
		</Block>
	</Sheet>
{/if}
=======
		<wa-icon class="text-xl" src={wrapPathInSvg(mdiDotsHorizontal)}></wa-icon>
	</IconButton>
</div>
>>>>>>> edit-message-frontend
