<script lang="ts">
	import type { Snippet } from 'svelte';
	import { page } from '$app/state';
	import ChatListPanel from './ChatListPanel.svelte';
	import SettingsPanel from './SettingsPanel.svelte';
	import NewMessagePanel from './NewMessagePanel.svelte';
	import EmptyState from './EmptyState.svelte';

	let { children }: { children: Snippet } = $props();

	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
	const isNewMessage = $derived(
		page.url.pathname.startsWith('/new-message') ||
			page.state.sidebarPanel === 'new-message',
	);
	const isSidebarRoute = $derived(
		page.url.pathname === '/' ||
			page.url.pathname === '/settings' ||
			page.url.pathname === '/new-message',
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
	<div class="desktop-content" class:desktop-content-settings={isSettings}>
		{#if isSidebarRoute}
			<EmptyState />
		{:else}
			{@render children()}
		{/if}
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

	.desktop-content-settings :global(.k-navbar:not(:has(.k-navbar-back-link))) {
		padding-left: 12px;
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
