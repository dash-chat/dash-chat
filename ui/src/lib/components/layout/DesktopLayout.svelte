<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import ChatListPanel from './ChatListPanel.svelte';
	import SettingsPanel from './SettingsPanel.svelte';
	import NewMessagePanel from './NewMessagePanel.svelte';

	let { children }: { children: Snippet } = $props();

	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
	const isNewMessage = $derived(
		page.url.pathname === '/new-message' || page.url.pathname === '/add-contact',
	);
</script>

<div class="desktop-layout">
	<div class="desktop-sidebar">
		{#if isSettings}
			<SettingsPanel />
		{:else if isNewMessage}
			<NewMessagePanel />
		{:else}
			<ChatListPanel />
		{/if}
	</div>
	<div class="desktop-content">
		{@render children()}
	</div>
</div>

<style>
	.desktop-layout {
		display: flex;
		height: 100vh;
		width: 100%;
	}

	.desktop-sidebar {
		width: 320px;
		min-width: 320px;
		border-right: 1px solid var(--k-hairline-color);
		overflow-y: auto;
		overflow-x: hidden;
		background-color: var(--k-color-md-light-surface);
	}

	.desktop-content {
		flex: 1;
		min-width: 0;
		position: relative;
		overflow: hidden;
		background-color: var(--k-color-md-light-surface);
	}

	/* Disable center-in-desktop constraint — the right panel already provides width */
	.desktop-content :global(.center-in-desktop) {
		width: 100%;
		align-self: stretch;
	}

	@media (prefers-color-scheme: dark) {
		.desktop-sidebar {
			background-color: var(--k-color-md-dark-surface);
		}
		.desktop-content {
			background-color: var(--k-color-md-dark-surface);
		}
	}
</style>
