<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ContactsStore, ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { page, navigating } from '$app/state';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import ChatListPanel from './ChatListPanel.svelte';
	import SettingsPanel from './SettingsPanel.svelte';
	import NewMessagePanel from './NewMessagePanel.svelte';
	import EmptyState from './EmptyState.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';

	let { children }: { children: Snippet } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');
	const contacts = useReactivePromise(contactsStore.contactsAgentIds);
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);

	const isHome = $derived(page.url.pathname === '/');
	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
	const isNewMessage = $derived(
		page.url.pathname.startsWith('/new-message') ||
			page.state.sidebarPanel === 'new-message',
	);
	const isNavigatingToSidebarRoute = $derived(
		navigating.to?.url.pathname === '/' ||
			navigating.to?.url.pathname === '/settings' ||
			navigating.to?.url.pathname === '/new-message',
	);
	const isSidebarRoute = $derived(
		isNavigatingToSidebarRoute ||
			!page.url?.pathname ||
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
			{#if isHome}
				{#await $contacts then contactsList}
				{#await $chatSummaries then chats}
					{@const showGetStarted = contactsList.length === 0 && chats.length === 0}
					{#if showGetStarted}
						<div class="absolute bottom-0 left-0 right-0 z-10">
							<GetStarted />
						</div>
					{/if}
				{/await}
			{/await}
			{/if}
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
		width: 280px;
		min-width: 280px;
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

	:global(.dark) .desktop-sidebar {
		background-color: var(--k-color-md-dark-surface);
	}
	:global(.dark) .desktop-content {
		background-color: var(--k-color-md-dark-surface);
	}
</style>
