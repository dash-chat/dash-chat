<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronDown } from '@mdi/js';
	import { Badge } from 'konsta/svelte';

	interface Props {
		unreadCount?: number;
		onClick: () => void;
	}

	let { unreadCount = 0, onClick }: Props = $props();
</script>

<button
	class="relative flex h-10 w-10 items-center justify-center rounded-full bg-gray-100 shadow-md transition-opacity hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600"
	onclick={onClick}
	aria-label="Scroll to bottom"
	data-testid="direct-chat-scroll-bottom"
>
	{#if unreadCount > 0}
		<Badge
			class="absolute -top-1 -right-1"
			data-testid="direct-chat-unread-badge"
		>
			{unreadCount > 99 ? '99+' : unreadCount}
		</Badge>
	{/if}
	<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
</button>
