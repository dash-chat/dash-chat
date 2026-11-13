<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import {
		fullName,
		type ContactsStore,
		type SettingsStore,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import {
		mdiAccountCircleOutline,
		mdiBellOutline,
		mdiQrcode,
		mdiPaletteOutline,
		mdiHelpCircleOutline,
		mdiServerOutline,
	} from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { showAddContactInContentPanel } from '$lib/utils/add-contact-content-panel';
	import {
		Link,
		List,
		ListItem,
		Navbar,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import TitleTruncatedListItem from '$lib/components/TitleTruncatedListItem.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isIos } from '$lib/utils/environment';
	import Avatar from '../profiles/Avatar.svelte';
	import WarningIcon from '../WarningIcon.svelte';

	const settingsStore: SettingsStore = getContext('settings-store');
	const localHubBlocked = useReactiveValue(settingsStore.localHubBlocked);

	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
	const theme = $derived(useTheme());

	const isActive = (path: string) => page.url.pathname.startsWith(path);
</script>

<div class="settings-panel">
	<Navbar title={m.settings()} titleClass="opacity1" transparent={true}>
		{#snippet right()}
			<Link
				iconOnly
				aria-label={m.addContact()}
				data-testid="settings-qr-link"
				onClick={() =>
					isWideScreen.value
						? showAddContactInContentPanel()
						: goto('/settings/profile/add-contact')}
			>
				<wa-icon src={wrapPathInSvg(mdiQrcode)}> </wa-icon>
			</Link>
		{/snippet}
	</Navbar>

	{#await $myProfile}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then myProfile}
		<List
			strongIos
			nested={theme === 'material'}
			inset={isWideScreen.value || theme === 'ios'}
		>
			<TitleTruncatedListItem
				link
				class={isActive('/settings/profile') ? 'active' : ''}
				chevron={false}
				linkProps={{ href: '/settings/profile' }}
				data-testid="settings-profile-link"
				title={myProfile ? fullName(myProfile) : undefined}
				titleFontSizeIos="text-xl"
				titleFontSizeMaterial="text-xl"
			>
				{#snippet media()}
					<Avatar
						image={myProfile?.avatar}
						initials={myProfile?.name.slice(0, 2)}
						style={isWideScreen.value || theme === 'ios'
							? '--size: 64px'
							: '--size: 64px; margin-inline-start: 6px'}
					/>
				{/snippet}
			</TitleTruncatedListItem>
		</List>

		<List strongIos nested inset={isWideScreen.value || theme === 'ios'}>
			<ListItem
				link
				class={isActive('/settings/account') ? 'active' : ''}
				linkProps={{ href: '/settings/account' }}
				data-testid="settings-account-link"
				title={m.account()}
				chevron={false}
			>
				{#snippet media()}
					<wa-icon
						src={wrapPathInSvg(mdiAccountCircleOutline)}
						style="font-size: 28px"
					></wa-icon>
				{/snippet}
			</ListItem>
			<ListItem
				link
				class={isActive('/settings/appearance') ? 'active' : ''}
				linkProps={{ href: '/settings/appearance' }}
				data-testid="settings-appearance-link"
				title={m.appearance()}
				chevron={false}
			>
				{#snippet media()}
					<wa-icon
						src={wrapPathInSvg(mdiPaletteOutline)}
						style="font-size: 28px"
					></wa-icon>
				{/snippet}
			</ListItem>
			<ListItem
				link
				class={isActive('/settings/notifications') ? 'active' : ''}
				linkProps={{ href: '/settings/notifications' }}
				data-testid="settings-notifications-link"
				title={m.notifications()}
				chevron={false}
			>
				{#snippet media()}
					<wa-icon src={wrapPathInSvg(mdiBellOutline)} style="font-size: 28px"
					></wa-icon>
				{/snippet}
			</ListItem>
		</List>

		{#if !isIos}
			<List
				strongIos
				nested={theme !== 'ios'}
				class={theme === 'ios' ? 'mb-0' : ''}
				inset={isWideScreen.value || theme === 'ios'}
			>
				<ListItem
					link
					class={isActive('/settings/offline') ? 'active' : ''}
					linkProps={{ href: '/settings/offline' }}
					data-testid="settings-offline-link"
					title={m.offlineFunctionality()}
					chevron={false}
				>
					{#snippet media()}
						<wa-icon
							src={wrapPathInSvg(mdiServerOutline)}
							style="font-size: 28px"
						></wa-icon>
					{/snippet}
					{#snippet after()}
						{#if $localHubBlocked === true}
							<WarningIcon
								size={24}
								label={m.localHubCantBeFound()}
								color="text-red-500"
								data-testid="settings-offline-hub-warning"
							/>
						{/if}
					{/snippet}
				</ListItem>
			</List>
		{/if}

		<List
			strongIos
			nested={theme !== 'ios'}
			inset={isWideScreen.value || theme === 'ios'}
		>
			<ListItem
				link
				class={isActive('/settings/help') ? 'active' : ''}
				linkProps={{ href: '/settings/help' }}
				data-testid="settings-help-link"
				title={m.help()}
				chevron={false}
			>
				{#snippet media()}
					<wa-icon
						src={wrapPathInSvg(mdiHelpCircleOutline)}
						style="font-size: 28px"
					></wa-icon>
				{/snippet}
			</ListItem>
		</List>
	{/await}
</div>

<style>
	.settings-panel {
		display: flex;
		flex-direction: column;
	}
</style>
