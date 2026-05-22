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

	// Stash the input that had focus *before* the tap so we can restore it
	// after `onClick` runs — keeps the soft keyboard up while scrolling to
	// the bottom. `pointerdown.preventDefault()` would also prevent focus
	// transfer, but on iOS it suppresses the synthesized `click`, so the
	// scroll-to-bottom action would never run.
	let previouslyFocused: HTMLElement | null = null;

	function rememberFocus() {
		const a = document.activeElement;
		previouslyFocused =
			a instanceof HTMLElement &&
			(a.tagName === 'TEXTAREA' || a.tagName === 'INPUT' || a.isContentEditable)
				? a
				: null;
	}

	function handleClick() {
		onClick();
		if (previouslyFocused && previouslyFocused !== document.activeElement) {
			previouslyFocused.focus({ preventScroll: true });
		}
		previouslyFocused = null;
	}
</script>

<button
	class="relative flex h-10 w-10 items-center justify-center rounded-full bg-gray-100 shadow-md transition-opacity hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600"
	onclick={handleClick}
	onpointerdown={rememberFocus}
	aria-label={m.scrollToBottom()}
	data-testid="direct-chat-scroll-bottom"
>
	{#if unreadCount > 0}
		<Badge
			class="absolute -top-1 -end-1"
			data-testid="direct-chat-unread-badge"
		>
			{unreadCount > 99 ? '99+' : unreadCount}
		</Badge>
	{/if}
	<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
</button>
