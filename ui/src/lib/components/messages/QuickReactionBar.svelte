<script lang="ts">
	import { QUICK_EMOJIS, condenseReactions } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal } from '@mdi/js';
	import { Popover, Sheet, Block, Chip } from 'konsta/svelte';
	import { getContext } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
	import IconButton from '$lib/components/IconButton.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';
	import { toggleReaction } from '$lib/utils/reactions';

	interface Props {
		message: Message;
		/** The message bubble the popover anchors to. */
		target: HTMLElement | undefined;
		myDeviceId: DeviceId;
		onClose: () => void;
	}

	let { message, target, myDeviceId, onClose }: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let expanded = $state(false);

	// Collapse the picker before unmounting, otherwise Konsta leaves the open
	// sheet orphaned in the DOM.
	function close() {
		if (expanded) {
			expanded = false;
			setTimeout(onClose, 300);
		} else {
			onClose();
		}
	}

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}

	function react(emoji: string) {
		toggleReaction(store, message, myDeviceId, emoji);
		close();
	}

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
</script>

<Popover
	opened={!expanded}
	{target}
	onBackdropClick={close}
	class="!w-auto !rounded-full"
>
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
				onclick={() => react(emoji)}
				data-testid={`quick-reaction-${emoji}`}
			>
				{emoji}
			</button>
		{/each}
		<IconButton
			icon={mdiDotsHorizontal}
			onClick={() => (expanded = true)}
			label={m.moreReactions()}
			testid="quick-reaction-more"
			iconClass="text-xl"
		/>
	</div>
</Popover>

<Sheet class="pb-safe text-lg" opened={expanded} onBackdropClick={onClose}>
	<div class="flex flex-col items-center">
		<SheetHandle />
	</div>
	{#if condensed.length > 0}
		<Block>
			{#each condensed as reaction}
				<button class="me-2 text-lg" onclick={() => react(reaction.emoji)}>
					<Chip class="border !border-white dark:!border-black">
						{reaction.emoji}{#if reaction.count > 1}&nbsp;{reaction.count}{/if}
					</Chip>
				</button>
			{/each}
		</Block>
	{/if}
	<Block>
		<EmojiPickerWrapper onEmojiSelected={react}></EmojiPickerWrapper>
	</Block>
</Sheet>
