<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { page } from '$app/state';
	import { mdiChat, mdiChatOutline, mdiCog, mdiCogOutline } from '@mdi/js';
	import { Tabbar, TabbarLink, ToolbarPane } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
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
				<wa-icon
					src={wrapPathInSvg(isSettings ? mdiChatOutline : mdiChat)}
					style="font-size: 24px"
				></wa-icon>
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
					<wa-icon
						src={wrapPathInSvg(isSettings ? mdiCog : mdiCogOutline)}
						style="font-size: 24px"
					></wa-icon>
				</HubWarningBadge>
			{/snippet}
		</TabbarLink>
	</ToolbarPane>
</Tabbar>
