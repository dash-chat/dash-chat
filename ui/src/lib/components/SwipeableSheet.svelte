<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { Sheet } from 'konsta/svelte';
	import { registerAboveKeyboard } from 'tauri-plugin-virtual-keyboard';
	import { swipeToClose } from '$lib/utils/swipe-to-close';

	interface Props {
		opened: boolean;
		onClose: () => void;
		class?: string;
		colors?: { bgIos?: string; bgMaterial?: string };
		backdrop?: boolean;
		/** Glide the sheet above the keyboard while it is open, for sheets that
		 * hold a text input. */
		aboveKeyboard?: boolean;
		children: Snippet;
	}

	let {
		opened,
		onClose,
		class: className = '',
		colors,
		backdrop = true,
		aboveKeyboard = false,
		children,
	}: Props = $props();

	let anchor: HTMLElement;
	let sheet = $state<HTMLElement>();

	onMount(() => {
		const found = anchor.closest<HTMLElement>('.k-sheet');
		if (!found) throw new Error('SwipeableSheet must render inside a .k-sheet');
		sheet = found;
		return swipeToClose(found, () => onClose());
	});

	// A swipe closes the sheet with its drag offset still applied; drop it
	// before the sheet slides back in.
	$effect.pre(() => {
		if (!opened || !sheet) return;
		sheet.style.transition = '';
		sheet.style.transform = '';
	});

	$effect(() => {
		if (aboveKeyboard && opened && sheet) return registerAboveKeyboard(sheet);
	});
</script>

<Sheet class={className} {colors} {opened} {backdrop} onBackdropClick={onClose}>
	<!-- `contents` keeps the sheet's own layout: children stay its direct flex items. -->
	<div bind:this={anchor} class="contents">
		{@render children()}
	</div>
</Sheet>
