<script lang="ts">
	import type { ChatsStore } from 'dash-chat-stores';
	import { Badge, useTheme } from 'konsta/svelte';
	import { getContext, type Snippet } from 'svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';

	let { children }: { children: Snippet } = $props();

	const MAX_DISPLAYED_COUNT = 99;

	const chatsStore: ChatsStore = getContext('chats-store');
	const unreadMessagesCount = useReactiveValue(chatsStore.unreadMessagesCount);
	const count = $derived($unreadMessagesCount ?? 0);

	const theme = $derived(useTheme());
	const position = $derived(
		theme === 'ios'
			? '-top-1.75 start-[calc(100%-8px)]'
			: '-top-2 start-[calc(100%-9px)]',
	);
</script>

<span class="relative inline-flex">
	{@render children()}
	{#if count > 0}
		<Badge
			class="absolute leading-none {position}"
			data-testid="chats-unread-badge"
		>
			{count > MAX_DISPLAYED_COUNT ? `${MAX_DISPLAYED_COUNT}+` : count}
		</Badge>
	{/if}
</span>
