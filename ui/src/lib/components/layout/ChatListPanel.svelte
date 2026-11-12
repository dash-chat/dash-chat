<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiQrcode, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/chats/AllChats.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';

	import { Link, Navbar, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { pushState } from '$app/navigation';

	const theme = $derived(useTheme());
</script>

<div class="flex flex-col relative h-full">
	<Navbar title={m.chats()} titleClass="opacity1" transparent={true}>
		{#snippet right()}
			<Link
				iconOnly
				href="/new-message/add-contact"
				aria-label={m.addContact()}
				data-testid="home-add-contact-btn"
			>
				<wa-icon src={wrapPathInSvg(mdiQrcode)}> </wa-icon>
			</Link>
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
