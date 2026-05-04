<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPencil, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/AllChats.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';
	import FirstChatTooltip from '$lib/components/FirstChatTooltip.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';
	import { Fab, Link, Navbar, Page, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	const theme = $derived(useTheme());

	let getStartedVisible = $state(true);
	const contactsStore: ContactsStore = getContext('contacts-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);

	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);
</script>

<Page>
	<Navbar title={m.chats()} titleClass="opacity1" rightClass="relative" transparent={true}>
		{#snippet left()}
			{#await $myProfile then myProfile}
				<Link iconOnly href="/settings" data-testid="home-settings-link">
					<Avatar
						image={myProfile?.avatar}
						initials={myProfile?.name.slice(0, 2)}
						style="--size: 42px"
					/>
				</Link>
			{/await}
		{/snippet}

		{#snippet right()}
			{#if theme === 'ios'}
				<Link iconOnly href="/new-message" data-testid="home-new-message-link">
					<wa-icon src={wrapPathInSvg(mdiSquareEditOutline)}> </wa-icon>
				</Link>
				{#if !isWideScreen.value}
					{#await $chatSummaries then chats}
						{#if chats.length === 0}
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
		<div class="flex flex-col fixed bottom-4 left-0 right-0 z-10 pb-safe">
			{#if theme == 'material'}
				{#await $chatSummaries then chats}
					{#if chats.length === 0}
						<div class="self-end me-4 mb-2 z-30">
							<FirstChatTooltip />
						</div>
					{/if}
				{/await}
				<Fab
					class="z-20 me-4"
					style="align-self: end;"
					onClick={() => goto('/new-message')}
					data-testid="home-new-message-fab"
				>
					<wa-icon src={wrapPathInSvg(mdiPencil)}> </wa-icon>
				</Fab>
			{/if}
			<GetStarted bind:visible={getStartedVisible} />
		</div>
	{/if}
</Page>
