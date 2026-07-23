<script lang="ts">
	import type { Snippet } from 'svelte';
	import TestBanner from './TestBanner.svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="mobile-shell">
	<TestBanner />
	<div class="mobile-content no-safe-areas-top">
		{@render children()}
	</div>
</div>

<style>
	.mobile-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
		width: 100%;
		/* The keyboard overlays the webview without resizing it; the plugin sets
		   --keyboard-inset-height once per transition, so this padding reserves the
		   keyboard's space on every screen. Nodes that should glide into that space
		   rather than jump register with renderAboveKeyboard. */
		padding-bottom: var(--keyboard-inset-height, 0px);
	}

	.mobile-content {
		position: relative;
		flex: 1;
		min-height: 0;
	}
</style>
