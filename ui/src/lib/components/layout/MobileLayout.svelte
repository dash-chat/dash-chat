<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import MainTabbar from './MainTabbar.svelte';
	import TestBanner from './TestBanner.svelte';

	let { children }: { children: Snippet } = $props();

	// Non-special URL schemes (tauri://) have empty pathname for the root,
	// unlike http:// which normalizes to '/'.
	const pathname: string = $derived(page.url.pathname);
	const isTabRoot = $derived(
		pathname === '/' || pathname === '' || pathname === '/settings',
	);
	let tabbarHeight = $state(0);
</script>

<div class="mobile-shell pb-keyboard-safe">
	<TestBanner />
	<div
		class="mobile-content no-safe-areas-top"
		class:has-main-tabbar={isTabRoot}
		style:--main-tabbar-height="{tabbarHeight}px"
	>
		{@render children()}
		{#if isTabRoot}
			<div
				class="absolute bottom-0 inset-x-0 z-20"
				bind:clientHeight={tabbarHeight}
			>
				<MainTabbar />
			</div>
		{/if}
	</div>
</div>

<style>
	.mobile-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
		width: 100%;
		overflow: clip;
	}

	.mobile-content {
		position: relative;
		flex: 1;
		min-height: 0;
	}

	.has-main-tabbar :global(.k-page) {
		padding-bottom: var(--main-tabbar-height);
	}
</style>
