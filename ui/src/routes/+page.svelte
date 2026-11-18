<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPencil, mdiSquareEditOutline } from '@mdi/js';
	import AllChats from '$lib/components/chats/AllChats.svelte';
	import AddContactNavbarButton from '$lib/components/contacts/AddContactNavbarButton.svelte';
	import GetStarted from '$lib/components/GetStarted.svelte';
	import FirstChatTooltip from '$lib/components/FirstChatTooltip.svelte';
	import ConnectionStatusIndicator from '$lib/components/connection/ConnectionStatusIndicator.svelte';
	import UpdaterBanner from '$lib/components/UpdaterBanner.svelte';
	import { Fab, Link, Navbar, Page, useTheme } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import { isWideScreen } from '$lib/stores/screen.svelte';

	const theme = $derived(useTheme());

	let scrollEl: HTMLDivElement | null = $state(null);
	let navbarHeight = $state(0);
	// Konsta's iOS navbar height, so the list sits clear of it before it is
	// measured.
	const navbarFallbackHeight = 'calc(max(16px, var(--k-safe-area-top)) + 44px)';

	let getStartedVisible = $state(true);
	let getStartedHeight = $state(0);
	const chatsStore: ChatsStore = getContext('chats-store');
	const chatSummaries = useReactivePromise(chatsStore.allChatsSummaries);
	const loadedChatSummaries = useReactiveValue(chatsStore.allChatsSummaries);
	const listNeedsFabClearance = $derived(
		theme === 'material' &&
			!isWideScreen.value &&
			($loadedChatSummaries?.length ?? 0) > 0,
	);
</script>

{#snippet connectionStatus()}
	<div class="grid size-11 place-items-center">
		<ConnectionStatusIndicator />
	</div>
{/snippet}

{#snippet materialActions()}
	<AddContactNavbarButton />
{/snippet}

{#snippet composeActions()}
	<AddContactNavbarButton />
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

{#snippet chatList()}
	<UpdaterBanner />

	<AllChats
		class="flex flex-1 flex-col {listNeedsFabClearance
			? 'pb-22'
			: 'pb-(--get-started-clearance)'}"
	></AllChats>
{/snippet}

{#snippet bottomActions()}
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
			<div bind:clientHeight={getStartedHeight}>
				<GetStarted bind:visible={getStartedVisible} />
			</div>
		</div>
	{/if}
{/snippet}

{#if theme === 'ios'}
	<Page
		class="!overflow-hidden"
		style="--get-started-clearance: {getStartedHeight > 0
			? getStartedHeight + 16
			: 0}px"
	>
		<!-- The list scrolls in its own container under the navbar: inside the
			scrolling page, iOS's overscroll bounce dragged the navbar along. -->
		<div
			bind:this={scrollEl}
			class="absolute inset-0 flex flex-col overflow-y-auto pb-(--main-tabbar-height)"
			style:padding-top={navbarHeight > 0
				? `${navbarHeight}px`
				: navbarFallbackHeight}
		>
			<h1 class="flex h-13 items-center px-safe-4 mt-2 text-[34px] font-bold">
				{m.chats()}
			</h1>
			{@render chatList()}
		</div>

		<div class="absolute inset-x-0 top-0 z-20" bind:clientHeight={navbarHeight}>
			<!-- Konsta only attaches its scroll listener on mount. It fades the
				bar's title in as the large title scrolls under the bar. -->
			{#if scrollEl}
				<Navbar
					{scrollEl}
					title={m.chats()}
					left={connectionStatus}
					right={composeActions}
					transparent={true}
				/>
			{/if}
		</div>

		{@render bottomActions()}
	</Page>
{:else}
	<Page
		class="flex flex-col"
		style="--get-started-clearance: {getStartedHeight > 0
			? getStartedHeight + 16
			: 0}px"
	>
		<Navbar
			title={m.chats()}
			titleClass="opacity1"
			left={connectionStatus}
			right={materialActions}
			transparent={true}
		/>

		{@render chatList()}

		{@render bottomActions()}
	</Page>
{/if}
