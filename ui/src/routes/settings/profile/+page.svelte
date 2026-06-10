<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { mdiAccount, mdiInformationOutline, mdiQrcode } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { m } from '$lib/paraglide/messages.js';
	import { fullName } from 'dash-chat-stores';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import {
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import EditableAvatar from '$lib/components/profiles/EditableAvatar.svelte';
	import TitleTruncatedListItem from '$lib/components/TitleTruncatedListItem.svelte';

	const theme = $derived(useTheme());
	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
</script>

<Page>
	<Navbar title={m.profile()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#if !isWideScreen.value}
				<NavbarBackLink
					onClick={() => goto('/settings')}
					data-testid="profile-back"
				/>
			{/if}
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
				<EditableAvatar
					image={myProfile?.avatar}
					initials={myProfile?.name.slice(0, 2)}
					editUrl="/settings/profile/edit-photo"
					class="mt-2 mb-4"
				/>

				<List nested strongIos inset={isWideScreen.value || theme === 'ios'}>
					<TitleTruncatedListItem
						title={fullName(myProfile!)}
						link
						linkProps={{ href: '/settings/profile/edit-name' }}
						data-testid="profile-edit-name"
						chevronMaterial={false}
					>
						{#snippet media()}
							<wa-icon src={wrapPathInSvg(mdiAccount)}></wa-icon>
						{/snippet}
					</TitleTruncatedListItem>
					<ListItem
						title={m.about()}
						link
						linkProps={{ href: '/settings/profile/edit-about' }}
						data-testid="profile-edit-about"
						chevronMaterial={false}
					>
						{#snippet media()}
							<wa-icon src={wrapPathInSvg(mdiInformationOutline)}></wa-icon>
						{/snippet}
					</ListItem>
				</List>

				<p class="explanation">{m.setProfileExplanation()}</p>

				<div class="divider"></div>

				<List nested strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListItem
						title={m.qrCodeOrLink()}
						link
						linkProps={{ href: '/settings/profile/add-contact' }}
						data-testid="profile-qr-link"
						chevronMaterial={false}
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
	}

	:global(.k-ios) .divider {
		margin: 0 16px 24px 16px;
	}

	:global(.k-material) .divider {
		margin: 0 16px 12px 16px;
	}
</style>
