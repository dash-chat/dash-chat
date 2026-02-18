<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { fullName, type ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import {
		mdiBellOutline,
		mdiMagnify,
		mdiPalette,
		mdiPlusCircle,
		mdiChevronRight,
	} from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { showToast } from '$lib/utils/toasts';
	import PeerProfileSheet from '$lib/components/PeerProfileSheet.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	let agentId = page.params.agentId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(agentId);

	const peerProfile = useReactivePromise(store.peerProfile);

	let showPeerProfile = $state(false);

	function comingSoon() {
		showToast(m.comingSoon());
	}
</script>

<Page>
	{#await $peerProfile}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then profile}
		<Navbar transparent={true}>
			{#snippet left()}
				<NavbarBackLink onClick={() => goto(`/direct-chats/${agentId}`)} data-testid="chat-settings-back" />
			{/snippet}
		</Navbar>

		<div class="column" style="flex: 1">
			<div class="column center-in-desktop">
				<div class="column m-6 gap-2" style="align-items: center">
					<wa-avatar
						image={profile?.avatar}
						initials={profile?.name.slice(0, 2)}
						style="--size: 80px;"
					>
					</wa-avatar>

					<button class="flex items-center gap-1" onclick={() => (showPeerProfile = true)}>
						<span class="text-xl font-semibold" data-testid="chat-settings-peer-name">{fullName(profile!)}</span>
						<wa-icon
							class="small-icon quiet"
							src={wrapPathInSvg(mdiChevronRight)}
						></wa-icon>
					</button>

					{#if profile?.about}
						<span class="quiet text-center">{profile.about}</span>
					{/if}
				</div>

				<div class="mb-4 flex justify-center gap-6">
					<!-- TODO: Coming soon - mute notifications -->
					{#if false}
					<div class="flex flex-col items-center gap-1">
						<Button class="icon-only" tonal onClick={comingSoon}>
							<wa-icon src={wrapPathInSvg(mdiBellOutline)}></wa-icon>
						</Button>
						<span class="text-xs">{m.mute()}</span>
					</div>
					{/if}
					<div class="flex flex-col items-center gap-1">
						<Button class="icon-only" tonal onClick={() => goto(`/direct-chats/${agentId}?search=true`)} data-testid="chat-settings-search-btn">
							<wa-icon src={wrapPathInSvg(mdiMagnify)}></wa-icon>
						</Button>
						<span class="text-xs">{m.search()}</span>
					</div>
				</div>

				<div class="mx-4 my-2 border-t border-gray-200 dark:border-gray-700"></div>

				<!-- TODO: Coming soon - chat color/wallpaper and groups in common -->
				{#if false}
				<List nested strongIos insetIos>
					<ListItem
						link
						chevron={false}
						title={m.chatColorAndWallpaper()}
						onClick={comingSoon}
					>
						{#snippet media()}
							<wa-icon
								style="font-size: 1.5rem;"
								src={wrapPathInSvg(mdiPalette)}
							></wa-icon>
						{/snippet}
					</ListItem>
				</List>

				<div class="mx-4 my-2 border-t border-gray-200 dark:border-gray-700"></div>

				<div class="mt-4 mb-1 px-4">
					<span class="font-bold">{m.noGroupsInCommonTitle()}</span>
				</div>

				<List nested strongIos insetIos>
					<ListItem
						link
						chevron={false}
						title={m.addToAGroup()}
						onClick={comingSoon}
					>
						{#snippet media()}
							<wa-icon
								style="font-size: 1.5rem;"
								src={wrapPathInSvg(mdiPlusCircle)}
							></wa-icon>
						{/snippet}
					</ListItem>
				</List>
				{/if}
			</div>
		</div>

		<PeerProfileSheet opened={showPeerProfile} onClose={() => (showPeerProfile = false)} {profile} />
	{/await}
</Page>
