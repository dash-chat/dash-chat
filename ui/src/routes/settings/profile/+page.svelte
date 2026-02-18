<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { mdiAccount, mdiInformationOutline, mdiQrcode } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import { fullName } from 'dash-chat-stores'
	import {
		Button,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
	} from 'konsta/svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
</script>

<Page>
	<Navbar title={m.profile()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/settings')} data-testid="profile-back" />
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
			<div class="column center-in-desktop">
				<div class="column m-10 gap-4" style="align-items: center">
					<wa-avatar
						image={myProfile?.avatar}
						initials={myProfile?.name.slice(0, 2)}
						style="--size: 80px;"
					>
					</wa-avatar>

					<Button tonal style="width: auto" rounded onClick={()=>goto('/settings/profile/edit-photo')} data-testid="profile-edit-photo">{m.editPhoto()}</Button>
				</div>

				<List nested strongIos insetIos>
					<ListItem
						title={fullName(myProfile!)}
						link
						linkProps={{ href: '/settings/profile/edit-name' }}
						data-testid="profile-edit-name"
					>
						{#snippet media()}
							<wa-icon src={wrapPathInSvg(mdiAccount)}></wa-icon>
						{/snippet}
					</ListItem>
					<ListItem
						title={m.about()}
						link
						linkProps={{ href: '/settings/profile/edit-about' }}
						data-testid="profile-edit-about"
					>
						{#snippet media()}
							<wa-icon src={wrapPathInSvg(mdiInformationOutline)}></wa-icon>
						{/snippet}
					</ListItem>
				</List>

				<p class="explanation">{m.setProfileExplanation()}</p>

				<div class="divider"></div>

				<List nested strongIos insetIos>
					<ListItem
						title={m.myQrCode()}
						link
						linkProps={{ href: '/add-contact' }}
						data-testid="profile-qr-link"
					>
						{#snippet media()}
							<wa-icon src={wrapPathInSvg(mdiQrcode)}></wa-icon>
						{/snippet}
					</ListItem>
				</List>

				<p class="explanation">{m.qrCodeExplanation()}</p>
			</div>
		</div>
	{/await}
</Page>

<style>
	.explanation {
		margin: 0;
		padding: 8px 16px 24px 16px;
		font-size: 14px;
		opacity: 0.6;
		line-height: 1.4;
	}

	.divider {
		height: 1px;
		background-color: var(--k-hairline-color, rgba(128, 128, 128, 0.3));
		margin: 0 16px 8px 16px;
	}
</style>
