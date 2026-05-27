<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/button/button.js';
	import { ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { List } from 'konsta/svelte';
	import { useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { previewFeatures } from '$lib/stores/preview-features.svelte';
	import ErrorPlaceholder from '../ErrorPlaceholder.svelte';
	import ChatSummary from './ChatSummary.svelte';

	let { class: className = '' }: { class?: string } = $props();

	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);

	const chatHref = (summary: { type: string; chatId: string }) =>
		summary.type === 'GroupChat'
			? `/group-chat/${summary.chatId}`
			: `/direct-chats/${summary.chatId}`;

	let activePath = $derived(page.url.pathname);

	const isActive = (summary: { type: string; chatId: string }) =>
		isWideScreen.value && activePath.startsWith(chatHref(summary));

	const theme = $derived(useTheme());
</script>

<div class={className}>
	{#await $chatSummaries then allSummaries}
		{@const summaries = allSummaries.filter(
			s => s.type !== 'GroupChat' || previewFeatures.enabled,
		)}
		{#if summaries.length > 0}
			<List
				nested
				inset={isWideScreen.value && theme === 'ios'}
				data-testid="all-chats-list"
			>
				{#each summaries as summary}
					<ChatSummary {summary} active={isActive(summary)} />
				{/each}
			</List>
		{:else}
			<div
				class="quiet flex flex-1 flex-col items-center justify-center pb-16 text-center"
				data-testid="all-chats-empty"
			>
				<p>{m.noChatsYet()}</p>
				<p>{m.noChatsYetSubtitle()}</p>
			</div>
		{/if}
	{:catch error}
		<ErrorPlaceholder message={m.errorUnexpected()} {error} />
	{/await}
</div>
