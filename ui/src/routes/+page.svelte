<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPencil, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/chats/AllChats.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';
	import FirstChatTooltip from '$lib/components/FirstChatTooltip.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';
	import { Fab, Link, Navbar, Page, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import ConnectionStatusIcon from '$lib/components/connection/ConnectionStatusIcon.svelte';
	import HubWarningBadge from '$lib/components/HubWarningBadge.svelte';

	const theme = $derived(useTheme());

	let getStartedVisible = $state(true);
	const contactsStore: ContactsStore = getContext('contacts-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);

	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);
</script>

<Page>
	<Navbar title={m.chats()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#await $myProfile then myProfile}
				<Link iconOnly href="/settings" data-testid="home-settings-link">
					<HubWarningBadge>
						<Avatar
							image={myProfile?.avatar}
							initials={myProfile?.name.slice(0, 2)}
							size={42}
						/>
					</HubWarningBadge>
				</Link>
			{/await}
		{/snippet}

		{#snippet right()}
			<!-- The same 44px cell as the compose button, so the two line up. On
				Material it stands alone at the edge, inset like the avatar. -->
			<div
				class="flex h-11 w-11 items-center justify-center {theme === 'material'
					? 'me-1'
					: ''}"
			>
				<ConnectionStatusIcon />
			</div>
			{#if theme === 'ios'}
				<Link iconOnly href="/new-message" data-testid="home-new-message-btn">
					<wa-icon src={wrapPathInSvg(mdiSquareEditOutline)}> </wa-icon>
				</Link>
				{#if !isWideScreen.value}
					{#await $chatSummaries then chats}
						{#if chats.length === 0}
							<!-- Absolute so it anchors to the navbar's inner row (already relative)
								and sits below the new-message icon without affecting layout. -->
							<div class="absolute end-0 top-full mt-2 z-30">
								<FirstChatTooltip />
							</div>
						{/if}
					{/await}
				{/if}
			{/if}
		{/snippet}
	</Navbar>

	<UpdaterBanner />

	<div class={theme === 'ios' ? 'mt-4' : ''}></div>

	<AllChats class="flex min-h-[70vh] flex-col"></AllChats>

	{#if !isWideScreen.value}
		<div
			class="flex flex-col fixed bottom-4 inset-x-0 z-10 pb-safe pointer-events-none"
		>
			{#if theme == 'material'}
				{#await $chatSummaries then chats}
					{#if chats.length === 0}
						<div class="self-end me-4 mb-2 z-30 pointer-events-auto">
							<FirstChatTooltip />
						</div>
					{/if}
				{/await}
				<Fab
					class="z-20 me-4 pointer-events-auto"
					style="align-self: end;"
					onClick={() => goto('/new-message')}
					data-testid="home-new-message-btn"
				>
					<wa-icon src={wrapPathInSvg(mdiPencil)}> </wa-icon>
				</Fab>
			{/if}
			<GetStarted bind:visible={getStartedVisible} />
		</div>
	{/if}
</Page>
