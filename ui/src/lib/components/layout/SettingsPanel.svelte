<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { mdiAccountCircleOutline, mdiQrcode } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import { page } from '$app/state';
	import {
		Link,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
	const theme = $derived(useTheme());

	const isActive = (path: string) => page.url.pathname.startsWith(path);
</script>

<div class="settings-panel">
	<Navbar title={m.settings()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/')} data-testid="settings-back" />
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
			<ListItem
				link
				chevron={false}
				linkProps={{ href: '/settings/profile' }}
				data-testid="settings-profile-link"
				title={myProfile?.name}
				class={isActive('/settings/profile') ? 'active' : ''}
			>
				{#snippet media()}
					<wa-avatar
						image={myProfile?.avatar}
						initials={myProfile?.name.slice(0, 2)}
						style={isWideScreen.value || theme === 'ios'
							? '--size: 64px'
							: '--size: 64px; margin-left: 16px'}
					>
					</wa-avatar>
				{/snippet}
				{#snippet after()}
					<div
						on:pointerdown|preventDefault|stopPropagation={(e: any) => {
							e.stopPropagation();
							e.preventDefault();
						}}
						style={isWideScreen.value || theme === 'ios' ? '' : 'margin: 16px'}
					>
						<Link
							iconOnly
							data-testid="settings-qr-link"
							onClick={e => {
								e.stopPropagation();
								e.preventDefault();
								goto('/settings/profile/add-contact');
							}}
						>
							<wa-icon src={wrapPathInSvg(mdiQrcode)}></wa-icon>
						</Link>
					</div>
				{/snippet}
			</ListItem>
		</List>

		<List strongIos nested inset={isWideScreen.value || theme === 'ios'}>
			<ListItem
				link
				linkProps={{ href: '/settings/account' }}
				data-testid="settings-account-link"
				title={m.account()}
				chevron={false}
				class={isActive('/settings/account') ? 'active' : ''}
			>
				{#snippet media()}
					<wa-icon
						src={wrapPathInSvg(mdiAccountCircleOutline)}
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
