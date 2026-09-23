<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Block } from 'konsta/svelte';
	import SwipeableSheet from '$lib/components/SwipeableSheet.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';

	interface Props {
		opened: boolean;
		onClose: () => void;
		onEmojiSelected: (emoji: string) => void;
		backdrop?: boolean;
		testid?: string;
		children?: Snippet;
	}

	let {
		opened,
		onClose,
		onEmojiSelected,
		backdrop = true,
		testid,
		children,
	}: Props = $props();

	let picker: EmojiPickerWrapper;

	// The sheet stays mounted while closed, so a search left open would
	// otherwise come back on the next open.
	$effect(() => {
		if (!opened) picker.clearSearch();
	});
</script>

<!-- Searching raises the keyboard; the sheet rides above it and the picker
     shrinks once the sheet reaches its cap, which stops short of the status bar
     so a downward swipe from the handle is not taken for the notification shade.
     The picker starts at emoji-picker-element's own default height (400px). -->
<SwipeableSheet
	class="flex max-h-[calc(100dvh-env(safe-area-inset-top,0px)-1.25rem)] flex-col pb-keyboard-safe text-lg"
	{opened}
	{onClose}
	{backdrop}
	aboveKeyboard
>
	<div class="flex flex-col items-center">
		<SheetHandle />
	</div>
	{@render children?.()}
	<Block class="h-100 min-h-0 [&_emoji-picker]:h-full" data-testid={testid}>
		<EmojiPickerWrapper bind:this={picker} {onEmojiSelected} />
	</Block>
</SwipeableSheet>
