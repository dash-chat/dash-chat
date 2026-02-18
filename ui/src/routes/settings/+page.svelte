<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { mdiAccountCircleOutline, mdiPencil, mdiQrcode } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Card,
		Link,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
		useTheme,
	} from 'konsta/svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
	const theme = $derived(useTheme());
</script>

<Page>
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
		<div class="column" style="flex: 1">
			<List
				class="center-in-desktop"
				strongIos
				nested={theme === 'material'}
				inset
			>
				<ListItem
					link
					chevron={false}
					linkProps={{ href: '/settings/profile' }}
					data-testid="settings-profile-link"
					title={myProfile?.name}
				>
					{#snippet media()}
						<wa-avatar
							image={myProfile?.avatar}
							initials={myProfile?.name.slice(0, 2)}
							style="--size: 64px"
						>
						</wa-avatar>
					{/snippet}
					{#snippet after()}
						<div
							on:pointerdown|preventDefault|stopPropagation={(e: any) => {
								e.stopPropagation();
								e.preventDefault();
							}}
						>
							<Link
								iconOnly
								data-testid="settings-qr-link"
								onClick={e => {
									e.stopPropagation();
									e.preventDefault();
									goto('/add-contact');
								}}
							>
								<wa-icon src={wrapPathInSvg(mdiQrcode)}></wa-icon>
							</Link>
						</div>
					{/snippet}
				</ListItem>
			</List>

			<List
				class="center-in-desktop"
				strongIos
				nested				inset
			>
				<ListItem
					link
					linkProps={{ href: '/settings/account' }}
					data-testid="settings-account-link"
					title={m.account()}
					chevron={false}
				>
					{#snippet media()}
						<wa-icon src={wrapPathInSvg(mdiAccountCircleOutline)} style="font-size: 28px"></wa-icon>
					{/snippet}
				</ListItem>
			</List>
		</div>
	{/await}
</Page>
