<script lang="ts">
	import type { Snippet } from 'svelte';
	import { useTheme } from 'konsta/svelte';

	interface Props {
		/** Whether the layer receives pointer events (the recording hint layer
		 * sits behind the active press gesture, so it must not). */
		pointerEvents?: boolean;
		children: Snippet;
	}

	let { pointerEvents = true, children }: Props = $props();

	const theme = $derived(useTheme());
</script>

<div
	class="voice-layer {pointerEvents ? '' : 'pointer-events-none'} {theme ===
	'ios'
		? 'bg-ios-light-glass backdrop-blur-lg dark:bg-ios-dark-glass'
		: 'bg-white dark:bg-gray-800'}"
>
	{@render children()}
</div>

<style>
	.voice-layer {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		border-radius: 22px;
	}
</style>
