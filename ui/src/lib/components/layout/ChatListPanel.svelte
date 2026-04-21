<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { type ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/AllChats.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';

	import { Link, Navbar, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { pushState } from '$app/navigation';
	import Avatar from '../profiles/Avatar.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);
	const theme = $derived(useTheme());
</script>

<div class="flex flex-col relative h-full">
	<Navbar title={m.chats()} titleClass="opacity1" transparent={true}>
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
			<Link
				iconOnly
				onClick={() => {
					pushState('', { sidebarPanel: 'new-message' });
				}}
				data-testid="home-new-message-link"
			>
				<wa-icon src={wrapPathInSvg(mdiSquareEditOutline)}> </wa-icon>
			</Link>
		{/snippet}
	</Navbar>

	<UpdaterBanner />

	<div class={theme === 'ios' ? 'mt-4' : ''}></div>

	<AllChats class="flex flex-1 flex-col"></AllChats>
</div>
