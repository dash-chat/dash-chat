<script lang="ts">
	import { QUICK_EMOJIS, condenseReactions } from '$lib/utils/emojis';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiDotsHorizontal } from '@mdi/js';
	import '@awesome.me/webawesome/dist/components/popover/popover.js';
	import type WaPopover from '@awesome.me/webawesome/dist/components/popover/popover.js';
	import { Sheet, Block, Chip } from 'konsta/svelte';
	import { getContext, onMount } from 'svelte';
	import type { Message, DeviceId, MessagesStore } from 'dash-chat-stores';
	import IconButton from '$lib/components/IconButton.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';
	import { toggleReaction } from '$lib/utils/reactions';

	interface Props {
		message: Message;
		/** id of the message bubble element the popover anchors to. */
		for: string;
		placement: WaPopover['placement'];
		myDeviceId: DeviceId;
		onClose: () => void;
	}

	let { message, for: forId, placement, myDeviceId, onClose }: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let popover = $state<WaPopover>();
	let open = $state(false);
	let expanded = $state(false);

	// Open only after the element's first render: WebAwesome registers its
	// outside-click / Escape dismiss handlers when `open` changes, but its watcher
	// skips the first update — so opening at mount-time would never be dismissable.
	onMount(() => {
		popover?.updateComplete.then(() => (open = true));
	});

	function expand() {
		open = false;
		expanded = true;
	}

	// The popover hides on outside click or Escape; tear down the reaction UI too.
	function onAfterHide() {
		if (!expanded) onClose();
	}

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}

	function react(emoji: string) {
		toggleReaction(store, message, myDeviceId, emoji);
		onClose();
	}

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
</script>

<wa-popover
	bind:this={popover}
	for={forId}
	{placement}
	{open}
	without-arrow
	onwa-after-hide={onAfterHide}
>
	<div
		class="flex items-center gap-1"
		role="group"
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
			onClick={expand}
			label={m.moreReactions()}
			testid="quick-reaction-more"
			iconClass="text-xl"
		/>
	</div>
</wa-popover>

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

<style>
	wa-popover::part(body) {
		padding: 0.375rem 0.5rem;
		border-radius: 9999px;
		background-color: white;
		box-shadow:
			0 10px 15px -3px rgb(0 0 0 / 0.1),
			0 4px 6px -4px rgb(0 0 0 / 0.1);
	}
	:global(html.dark) wa-popover::part(body) {
		background-color: #1f2937;
	}
</style>
