<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPencil, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/AllChats.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';
	import { Fab, Link, Navbar, Page, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	const theme = $derived(useTheme());

	let getStartedVisible = $state(true);
	const contactsStore: ContactsStore = getContext('contacts-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);
</script>

<Page>
	<Navbar title={m.chats()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			{#await $myProfile then myProfile}
				<Link iconOnly href="/settings" data-testid="home-settings-link">
					<wa-avatar
						image={myProfile?.avatar}
						initials={myProfile?.name.slice(0, 2)}
						style="--size: 42px"
					>
					</wa-avatar>
				</Link>
			{/await}
		{/snippet}

		{#snippet right()}
			{#if theme == 'ios'}
				<Link iconOnly href="/new-message" data-testid="home-new-message-link">
					<wa-icon src={wrapPathInSvg(mdiSquareEditOutline)}> </wa-icon>
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	<UpdaterBanner />

	<div class={theme === 'ios' ? 'mt-4' : ''}></div>

	<AllChats class="flex min-h-[70vh] flex-col"></AllChats>

	{#if !isWideScreen.value}
		<div class="fixed bottom-0 left-0 right-0 z-10 pb-safe">
			<GetStarted bind:visible={getStartedVisible} />
		</div>
	{/if}

	{#if theme == 'material' && !isWideScreen.value}
		<Fab
			class="fixed-action-btn z-20"
			onClick={() => goto('/new-message')}
			data-testid="home-new-message-fab"
		>
			<wa-icon src={wrapPathInSvg(mdiPencil)}> </wa-icon>
		</Fab>
	{/if}
</Page>
