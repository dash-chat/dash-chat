<script lang="ts">
	import type { ChatsStore } from 'dash-chat-stores';
	import { Badge } from 'konsta/svelte';
	import { getContext, type Snippet } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';

	let { children }: { children: Snippet } = $props();

	const MAX_DISPLAYED_COUNT = 99;

	const chatsStore: ChatsStore = getContext('chats-store');
	const unreadMessagesCount = useReactiveValue(chatsStore.unreadMessagesCount);
	const count = $derived($unreadMessagesCount ?? 0);
</script>

<span class="relative inline-flex">
	{@render children()}
	{#if count > 0}
		<Badge
			small={isWideScreen.value}
			class="absolute leading-none -top-1.25 start-full -ms-2.25"
			aria-hidden="true"
			data-testid="chats-unread-badge"
		>
			{count > MAX_DISPLAYED_COUNT ? `${MAX_DISPLAYED_COUNT}+` : count}
		</Badge>
		<span class="sr-only">{m.unreadMessages({ count })}</span>
	{/if}
</span>
