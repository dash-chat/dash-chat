<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
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
	import { onActivate } from '$lib/utils/keyboard';
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
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { page } from '$app/state';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	let agentId = page.params.agentId!;

	const theme = $derived(useTheme());
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
				<NavbarBackLink
					onClick={() => goto(`/direct-chats/${agentId}`)}
					data-testid="chat-settings-back"
				/>
			{/snippet}
		</Navbar>

		<div class="column" style="flex: 1">
			<div class="column center-in-desktop min-w-0">
				<div
					class="column m-6 gap-2 max-w-full px-4"
					style="align-items: center"
					data-testid="chat-settings-peer-header"
				>
					{#if profile}
						<Avatar
							image={profile.avatar}
							name={fullName(profile)}
							colorSeed={agentId}
							size={80}
						/>

						<div
							class="flex cursor-pointer items-center gap-1 max-w-full"
							onclick={() => (showPeerProfile = true)}
							onkeydown={onActivate(() => (showPeerProfile = true))}
							role="button"
							tabindex="0"
						>
							<span
								class="text-xl font-semibold break-words text-center min-w-0"
								data-testid="chat-settings-peer-name">{fullName(profile)}</span
							>
							<wa-icon
								class="small-icon quiet shrink-0"
								src={wrapPathInSvg(mdiChevronRight)}
							></wa-icon>
						</div>

						{#if profile.about}
							<span class="quiet text-center break-words max-w-full"
								>{profile.about}</span
							>
						{/if}
					{:else}
						<Avatar waitingForProfile size={80} />
						<span class="quiet text-xl">{m.waitingForProfile()}</span>
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
						<Button
							class="icon-only"
							tonal
							onClick={() => goto(`/direct-chats/${agentId}?search=true`)}
							data-testid="chat-settings-search-btn"
						>
							<wa-icon src={wrapPathInSvg(mdiMagnify)}></wa-icon>
						</Button>
						<span class="text-xs">{m.search()}</span>
					</div>
				</div>

				<div
					class="mx-4 my-2 border-t border-gray-200 dark:border-gray-700"
				></div>

				<!-- TODO: Coming soon - chat color/wallpaper and groups in common -->
				{#if false}
					<List nested strongIos inset={isWideScreen.value || theme === 'ios'}>
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

					<div
						class="mx-4 my-2 border-t border-gray-200 dark:border-gray-700"
					></div>

					<div class="mt-4 mb-1 px-4">
						<span class="font-bold">{m.noGroupsInCommonTitle()}</span>
					</div>

					<List nested strongIos inset={isWideScreen.value || theme === 'ios'}>
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

		{#if profile}
			<PeerProfileSheet
				opened={showPeerProfile}
				onClose={() => (showPeerProfile = false)}
				{profile}
				colorSeed={agentId}
			/>
		{/if}
	{/await}
</Page>
