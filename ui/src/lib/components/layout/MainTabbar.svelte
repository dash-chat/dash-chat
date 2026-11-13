<script lang="ts">
	import { page } from '$app/state';
	import { TabbarLink, Toolbar, ToolbarPane, useTheme } from 'konsta/svelte';
	import chatsSvg from '$lib/assets/main-tabbar/chats.svg?raw';
	import settingsSvg from '$lib/assets/main-tabbar/settings.svg?raw';
	import { m } from '$lib/paraglide/messages.js';
	import HubWarningBadge from '../HubWarningBadge.svelte';

	const isSettings = $derived(page.url.pathname.startsWith('/settings'));
	const theme = $derived(useTheme());
	// Konsta stretches the iOS bar across the screen; Signal's hugs its tabs.
	const hugTabs = $derived(theme === 'ios' ? 'w-auto!' : '');
	const tabWidth = $derived(theme === 'ios' ? 'min-w-28' : '');
</script>

<Toolbar tabbar tabbarLabels tabbarIcons innerClass={hugTabs}>
	<ToolbarPane class={hugTabs}>
		<TabbarLink
			class={tabWidth}
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
			class={tabWidth}
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
</Toolbar>
