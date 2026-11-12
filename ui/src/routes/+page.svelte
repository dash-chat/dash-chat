<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPencil, mdiQrcode, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/chats/AllChats.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';
	import FirstChatTooltip from '$lib/components/FirstChatTooltip.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';
	import { Fab, Link, Navbar, Page, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import ConnectionStatusIndicator from '$lib/components/connection/ConnectionStatusIndicator.svelte';

	const theme = $derived(useTheme());

	let getStartedVisible = $state(true);
	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);
</script>

{#snippet addContactAction()}
	<Link
		iconOnly
		href="/new-message/add-contact"
		aria-label={m.addContact()}
		data-testid="home-add-contact-btn"
	>
		<wa-icon src={wrapPathInSvg(mdiQrcode)}> </wa-icon>
	</Link>
{/snippet}

{#snippet composeActions()}
	{@render addContactAction()}
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
{/snippet}

<Page>
	<Navbar
		title={m.chats()}
		titleClass="opacity1"
		right={theme === 'ios' ? composeActions : addContactAction}
		transparent={true}
	>
		{#snippet left()}
			<div class="grid size-11 place-items-center">
				<ConnectionStatusIndicator />
			</div>
		{/snippet}
	</Navbar>

	<UpdaterBanner />

	<div class={theme === 'ios' ? 'mt-4' : ''}></div>

	<AllChats class="flex min-h-[70vh] flex-col"></AllChats>

	{#if !isWideScreen.value}
		<div
			class="flex flex-col fixed bottom-[calc(var(--main-tabbar-height)+1rem)] inset-x-0 z-10 pointer-events-none"
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
