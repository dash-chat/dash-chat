<script lang="ts">
	import type { Snippet } from 'svelte';
	import TestBanner from './TestBanner.svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="mobile-shell pb-keyboard-safe">
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
	}

	.mobile-content {
		position: relative;
		flex: 1;
		min-height: 0;
	}
</style>
