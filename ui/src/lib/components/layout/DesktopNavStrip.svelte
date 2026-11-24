<script lang="ts">
	import { page } from '$app/state';
	import chatsSvg from '$lib/assets/main-tabbar/chats.svg?raw';
	import settingsSvg from '$lib/assets/main-tabbar/settings.svg?raw';
	import { m } from '$lib/paraglide/messages.js';
	import ChatsUnreadBadge from '../ChatsUnreadBadge.svelte';
	import HubWarningBadge from '../HubWarningBadge.svelte';

	const isSettings = $derived(page.url.pathname.startsWith('/settings'));

	function tabClass(active: boolean): string {
		return `grid h-9 w-11 place-items-center rounded-lg text-md-light-on-surface transition-colors duration-200 dark:text-md-dark-on-surface ${
			active
				? 'bg-desktop-nav-strip-active-bg'
				: 'hover:bg-desktop-nav-strip-active-bg/60'
		}`;
	}
</script>

<nav
	data-testid="desktop-nav-strip"
	class="flex h-full w-15 shrink-0 flex-col items-center justify-between border-e border-(--k-hairline-color) bg-desktop-sidebar-bg py-3.5"
>
	<a
		href="/"
		data-testid="main-tab-chats"
		aria-current={isSettings ? undefined : 'page'}
		title={m.chats()}
		class={tabClass(!isSettings)}
	>
		<span class="sr-only">{m.chats()}</span>
		<ChatsUnreadBadge>
			<span class="block size-5 *:size-full">
				{@html chatsSvg}
			</span>
		</ChatsUnreadBadge>
	</a>
	<a
		href="/settings"
		data-testid="main-tab-settings"
		aria-label={m.settings()}
		aria-current={isSettings ? 'page' : undefined}
		title={m.settings()}
		class={tabClass(isSettings)}
	>
		<HubWarningBadge>
			<span class="block size-5 *:size-full">
				{@html settingsSvg}
			</span>
		</HubWarningBadge>
	</a>
</nav>
