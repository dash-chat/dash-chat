<script lang="ts">
	import type { ChatsStore } from 'dash-chat-stores';
	import { Badge, useTheme } from 'konsta/svelte';
	import { getContext, type Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { useReactiveValue } from '$lib/stores/use-signal';

	let { children }: { children: Snippet } = $props();

	const MAX_DISPLAYED_COUNT = 99;

	const chatsStore: ChatsStore = getContext('chats-store');
	const unreadMessagesCount = useReactiveValue(chatsStore.unreadMessagesCount);
	const count = $derived($unreadMessagesCount ?? 0);

	const theme = $derived(useTheme());
	// Anchored by its start edge, so a count of several digits grows away from
	// the icon instead of over it.
	const position = $derived(
		theme === 'ios'
			? '-top-1.75 start-full -ms-2'
			: '-top-2 start-full -ms-2.25',
	);
</script>

<span class="relative inline-flex">
	{@render children()}
	{#if count > 0}
		<Badge
			class="absolute leading-none {position}"
			aria-hidden="true"
			data-testid="chats-unread-badge"
		>
			{count > MAX_DISPLAYED_COUNT ? `${MAX_DISPLAYED_COUNT}+` : count}
		</Badge>
		<span class="sr-only">{m.unreadMessages({ count })}</span>
	{/if}
</span>
