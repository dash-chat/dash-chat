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
		/** Whether the reaction UI is showing — drives the popover open/close. */
		opened: boolean;
		/** The message bubble the popover anchors to. */
		target: HTMLElement | undefined;
		myDeviceId: DeviceId;
	}

	let { message, opened = $bindable(), target, myDeviceId }: Props = $props();

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

	function hasReacted(emoji: string): boolean {
		return message.reactions[myDeviceId] === emoji;
	}

	function react(emoji: string) {
		toggleReaction(store, message, myDeviceId, emoji);
		close();
	}

	const condensed = $derived(condenseReactions(message.reactions, myDeviceId));
</script>

<!-- The popover backdrop is the single, steady dim the whole time the reaction UI
     is up; while the picker sheet covers it, only the popover card is hidden (two
     cross-fading backdrops would dip lighter mid-transition). -->
<Popover
	{opened}
	{target}
	onBackdropClick={close}
	class={`!z-50 !w-auto !rounded-full ${expanded ? '!opacity-0 !pointer-events-none' : ''}`}
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
			class="!h-9 !w-9"
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
