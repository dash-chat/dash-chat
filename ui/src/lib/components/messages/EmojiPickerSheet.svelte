<script lang="ts">
	import { tick, type Snippet } from 'svelte';
	import { Block } from 'konsta/svelte';
	import { keyboard, onKeyboardWillHide } from 'tauri-plugin-virtual-keyboard';
	import SwipeableSheet from '$lib/components/SwipeableSheet.svelte';
	import SheetHandle from '$lib/components/SheetHandle.svelte';
	import EmojiPickerWrapper, {
		type SearchState,
	} from './EmojiPickerWrapper.svelte';
	import { useSignal } from '$lib/stores/use-signal';
	import { isMobile } from '$lib/utils/environment';

	const GLIDE_MS = 300;

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

	let search = $state<SearchState>({ focused: false, query: '' });
	// A typed query keeps the results up after the keyboard hides, and a tap on
	// a result must not collapse the sheet out from under the finger.
	const searching = $derived(search.focused || search.query !== '');
	let handleRow: HTMLElement;
	let picker: EmojiPickerWrapper;
	const keyboardHeight = useSignal(() => keyboard.height.value);
	const reservedKeyboardHeight = useSignal(() => keyboard.reservedHeight.value);

	// Not --keyboard-safe-bottom: the composer holds the keyboard's slot open
	// after the keyboard hides, which would leave a keyboard-sized gap here.
	// A focused search reserves the keyboard's full height up front, so the sheet
	// is laid out once instead of again when the keyboard reports its height.
	const bottomPadding = $derived(
		`max(env(safe-area-inset-bottom, 0px), ${
			search.focused && isMobile ? $reservedKeyboardHeight : $keyboardHeight
		}px)`,
	);

	/** Start `sheet` at `offset` from where it now sits and glide it into place. */
	function glideFrom(sheet: HTMLElement, offset: number) {
		sheet.style.transition = 'none';
		sheet.style.transform = `translateY(${offset}px)`;
		void sheet.offsetHeight;
		sheet.style.transition = `transform ${GLIDE_MS}ms cubic-bezier(0.2, 0, 0, 1)`;
		sheet.style.transform = '';
		const restoreTransition = () => (sheet.style.transition = '');
		sheet.addEventListener('transitionend', restoreTransition, { once: true });
	}

	async function updateSearch(next: SearchState) {
		if (!opened || !handleRow?.isConnected) {
			search = next;
			return;
		}
		const wasSearching = searching;
		const sheet = handleRow.closest<HTMLElement>('.k-sheet')!;
		const topBefore = sheet.getBoundingClientRect().top;
		search = next;
		await tick();
		if (searching !== wasSearching) {
			glideFrom(sheet, topBefore - sheet.getBoundingClientRect().top);
		}
	}

	// Android's back key hides the keyboard but leaves the search focused.
	$effect(() =>
		onKeyboardWillHide(() => {
			if (search.focused) picker.blurSearch();
		}),
	);

	// The sheet stays mounted while closed, so a search left open would
	// otherwise come back on the next open.
	$effect(() => {
		if (!opened) picker.clearSearch();
	});
</script>

<!-- Search results would sit under the keyboard, so searching takes the full
     screen, as in Signal. The extra top gap keeps the handle clear of the
     status bar, where Android takes a downward swipe for the notification shade. -->
<SwipeableSheet
	class="flex flex-col text-lg {searching
		? 'h-full pt-[calc(env(safe-area-inset-top,0px)+1.25rem)]'
		: ''}"
	{opened}
	{onClose}
	{backdrop}
	style="padding-bottom: {bottomPadding}"
>
	<div bind:this={handleRow} class="flex flex-col items-center">
		<SheetHandle />
	</div>
	{#if !searching}
		{@render children?.()}
	{/if}
	<Block
		class={searching ? 'min-h-0 flex-1 [&_emoji-picker]:h-full' : ''}
		data-testid={testid}
	>
		<EmojiPickerWrapper
			bind:this={picker}
			{onEmojiSelected}
			onSearchChange={updateSearch}
		/>
	</Block>
</SwipeableSheet>
