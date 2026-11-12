<script lang="ts">
	import { page } from '$app/state';
	import { Tabbar, TabbarLink, ToolbarPane } from 'konsta/svelte';
	import chatsSvg from '$lib/assets/main-tabbar/chats.svg?raw';
	import settingsSvg from '$lib/assets/main-tabbar/settings.svg?raw';
	import { m } from '$lib/paraglide/messages.js';
	import HubWarningBadge from '../HubWarningBadge.svelte';

	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
</script>

<Tabbar labels icons>
	<ToolbarPane>
		<TabbarLink
			active={!isSettings}
			label={m.chats()}
			linkProps={{ href: '/' }}
			data-testid="main-tab-chats"
		>
			{#snippet icon()}
				<span class="block size-6 *:size-full">
					{@html chatsSvg}
				</span>
			{/snippet}
		</TabbarLink>
		<TabbarLink
			active={isSettings}
			label={m.settings()}
			linkProps={{ href: '/settings' }}
			data-testid="main-tab-settings"
		>
			{#snippet icon()}
				<HubWarningBadge>
					<span class="block size-6 *:size-full">
						{@html settingsSvg}
					</span>
				</HubWarningBadge>
			{/snippet}
		</TabbarLink>
	</ToolbarPane>
</Tabbar>
