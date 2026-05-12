<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiChevronDown } from '@mdi/js';
	import { Badge } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		unreadCount?: number;
		onClick: () => void;
	}

	let { unreadCount = 0, onClick }: Props = $props();
</script>

<button
	class="relative flex h-10 w-10 items-center justify-center rounded-full bg-gray-100 shadow-md transition-opacity hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600"
	onclick={onClick}
	onpointerdown={e => {
		// Only suppress the default focus-transfer when a text input is
		// currently focused — otherwise we'd keep the page's focus exactly
		// where it was, which on iOS can re-show the keyboard if the user had
		// previously dismissed it without blurring the textarea.
		const a = document.activeElement;
		if (
			a instanceof HTMLElement &&
			(a.tagName === 'TEXTAREA' || a.tagName === 'INPUT' || a.isContentEditable)
		) {
			e.preventDefault();
		}
	}}
	aria-label={m.scrollToBottom()}
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
