<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Block } from 'konsta/svelte';
	import SwipeableSheet from '$lib/components/SwipeableSheet.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import { keyboard } from 'tauri-plugin-virtual-keyboard';
	import EmojiPickerWrapper from './EmojiPickerWrapper.svelte';
	import { useSignal } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';

	interface Props {
		opened: boolean;
		onClose: () => void;
		onEmojiSelected: (emoji: string) => void;
		backdrop?: boolean;
		onSearchFocus?: () => void;
		children?: Snippet;
	}

	let {
		opened,
		onClose,
		onEmojiSelected,
		backdrop = true,
		onSearchFocus,
		children,
	}: Props = $props();

	let picker: EmojiPickerWrapper | undefined;
	const reservedKeyboardHeight = useSignal(() =>
		isMobile ? keyboard.reservedHeight.value : 0,
	);

	// The sheet stays mounted while closed, so a search left open would
	// otherwise come back on the next open.
	$effect(() => {
		if (!opened) picker?.clearSearch();
	});
</script>

<!-- Searching raises the keyboard and the sheet rides above it, up to a cap that
     stops short of the status bar so a downward swipe from the handle is not
     taken for the notification shade. The cap leaves out the keyboard's usual
     height and adds back the keyboard padding in effect, so the content keeps
     one height whether the keyboard is up or down: the picker is sized for the
     keyboard from the start, and settling it reflows nothing. The picker is at
     most emoji-picker-element's own default height (400px). -->
<SwipeableSheet
	class="flex max-h-[calc(100dvh-env(safe-area-inset-top,0px)-1.25rem-var(--reserved-keyboard-height)+var(--keyboard-safe-bottom))] flex-col pb-keyboard-safe text-lg"
	style="--reserved-keyboard-height: {$reservedKeyboardHeight}px"
	{opened}
	{onClose}
	{backdrop}
	aboveKeyboard
>
	<div class="flex flex-col items-center">
		<SheetHandle />
	</div>
	{@render children?.()}
	<Block class="h-100 min-h-0 [&_emoji-picker]:h-full">
		<EmojiPickerWrapper bind:this={picker} {onEmojiSelected} {onSearchFocus} />
	</Block>
</SwipeableSheet>
