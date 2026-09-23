<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { Sheet } from 'konsta/svelte';
	import {
		registerAboveKeyboard,
		suppressKeyboardRestore,
	} from 'tauri-plugin-virtual-keyboard';
	import { swipeToClose } from '$lib/utils/swipe-to-close';
	import { portalToModalHost } from '$lib/actions/portal-to-modal-host';

	interface Props {
		opened: boolean;
		onClose: () => void;
		class?: string;
		style?: string;
		colors?: { bgIos?: string; bgMaterial?: string };
		backdrop?: boolean;
		/** Glide the sheet above the keyboard, for sheets that hold a text input. */
		aboveKeyboard?: boolean;
		children: Snippet;
	}

	let {
		opened,
		onClose,
		class: className = '',
		style,
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
		return swipeToClose(found, () => {
			// Closing drops the keyboard, and the keyboard glide measures the sheet
			// with whatever offset it carries: hand it back the plain position.
			if (aboveKeyboard) found.style.transform = '';
			onClose();
		});
	});

	// A swipe closes the other sheets with their drag offset still applied; drop it
	// before the sheet slides back in.
	$effect.pre(() => {
		if (!opened || !sheet) return;
		sheet.style.transition = '';
		sheet.style.transform = '';
	});

	// A keyboard restore requested while the sheet is open (a spotlight releasing
	// its slot hold) would pull focus behind it; it waits for the sheet to close.
	$effect(() => {
		if (opened) return suppressKeyboardRestore();
	});

	// Registered while mounted rather than only while open: closing drops focus
	// and so the keyboard, and unregistering during that glide would strand the
	// plugin's transform, leaving the closed sheet on screen.
	$effect(() => {
		if (aboveKeyboard && sheet) return registerAboveKeyboard(sheet);
	});
</script>

<div class="contents" {@attach portalToModalHost}>
	<Sheet
		class={className}
		{style}
		{colors}
		{opened}
		{backdrop}
		onBackdropClick={onClose}
	>
		<!-- `contents` keeps the sheet's own layout: children stay its direct flex items. -->
		<div bind:this={anchor} class="contents">
			{@render children()}
		</div>
	</Sheet>
</div>
