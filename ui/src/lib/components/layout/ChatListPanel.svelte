<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/chats/AllChats.svelte';
	import AddContactNavbarButton from '$lib/components/contacts/AddContactNavbarButton.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';

	import { Link, Navbar, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { pushState } from '$app/navigation';

	const theme = $derived(useTheme());
</script>

<div class="flex flex-col relative h-full">
	<Navbar
		title={m.chats()}
		titleClass="opacity1"
		rightClass="order-1"
		transparent={true}
	>
		{#if theme === 'ios'}
			<!-- A navbar child, not part of the right slot that Konsta wraps in one
				glass capsule: it gets its own bubble before the capsule (rightClass
				orders the slot last). -->
			<div class="ms-auto">
				<AddContactNavbarButton />
			</div>
		{/if}

		{#snippet right()}
			{#if theme !== 'ios'}
				<AddContactNavbarButton />
			{/if}
			<Link
				iconOnly
				onClick={() => {
					pushState('', { sidebarPanel: 'new-message' });
				}}
				data-testid="home-new-message-btn"
			>
				<wa-icon src={wrapPathInSvg(mdiSquareEditOutline)}> </wa-icon>
			</Link>
		{/snippet}
	</Navbar>

	<UpdaterBanner />

	<div class={theme === 'ios' ? 'mt-4' : ''}></div>

	<AllChats class="flex flex-1 flex-col"></AllChats>
</div>
