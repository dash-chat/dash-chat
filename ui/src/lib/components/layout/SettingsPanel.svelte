<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { fullName, type ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import {
		mdiAccountCircleOutline,
		mdiQrcode,
		mdiPaletteOutline,
		mdiHelpCircleOutline,
		mdiServerOutline,
	} from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import { page } from '$app/state';
	import {
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isMobile } from '$lib/utils/environment';
	import type { Action } from 'svelte/action';
	import Avatar from '../profiles/Avatar.svelte';

	const stopPropagation: Action = node => {
		const stop = (e: Event) => {
			e.stopPropagation();
			e.preventDefault();
		};
		node.addEventListener('pointerdown', stop);
		return {
			destroy() {
				node.removeEventListener('pointerdown', stop);
			},
		};
	};

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
							: '--size: 64px; margin-left: 16px'}
					/>
				{/snippet}
				{#snippet after()}
					<a
						href="/settings/profile/add-contact"
						class="qr-button"
						data-testid="settings-qr-link"
						use:stopPropagation
						style={isWideScreen.value || theme === 'ios' ? '' : 'margin: 16px'}
					>
						<wa-icon src={wrapPathInSvg(mdiQrcode)} style="font-size: 18px"
						></wa-icon>
					</a>
				{/snippet}
			</ListItem>
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
		</List>

		{#if !isMobile}
			<List strongIos nested inset={isWideScreen.value || theme === 'ios'}>
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

	.qr-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		border: none;
		background-color: var(--k-color-bg-300, rgba(128, 128, 128, 0.15));
		cursor: pointer;
		text-decoration: none;
		color: inherit;
		-webkit-tap-highlight-color: transparent;
	}

	.qr-button:active {
		opacity: 0.7;
	}
</style>
