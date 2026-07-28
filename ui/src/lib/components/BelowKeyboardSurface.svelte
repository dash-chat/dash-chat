<!--
	Render content below the keyboard — the counterpart to `renderAboveKeyboard`.
	When the keyboard hides, this surface is revealed in the freed space that the
	keyboard was occupying.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { watcher } from 'signalium';
	import {
		registerBelowKeyboard,
		type BelowKeyboardSurface,
	} from 'tauri-plugin-virtual-keyboard';

	interface Props {
		/** Open/close the surface, gliding everything registered above it. */
		open: boolean;
		class?: string;
		children: Snippet;
	}

	let { open, class: klass = '', children }: Props = $props();

	let node = $state<HTMLDivElement>();
	let surface = $state<BelowKeyboardSurface>();
	let visible = $state(false);

	$effect(() => {
		if (!node) return;
		const s = registerBelowKeyboard(node);
		surface = s;
		const w = watcher(() => s.visible.value);
		const unsubscribe = w.addListener(() => (visible = s.visible.value));
		visible = s.visible.value;
		return () => {
			unsubscribe();
			s.destroy();
			surface = undefined;
		};
	});

	$effect(() => {
		surface?.setOpen(open);
	});
</script>

<div bind:this={node} class="fixed bottom-0 inset-x-0 {klass}">
	{#if visible}
		{@render children()}
	{/if}
</div>
