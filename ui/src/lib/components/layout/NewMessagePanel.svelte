<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiAccountMultiplePlus,
		mdiAccountPlus,
		mdiDotsVertical,
	} from '@mdi/js';
	import {
		fullName,
		type AgentId,
		type ContactsStore,
		type Profile,
	} from 'dash-chat-stores';
	import { createAttachmentKey, fromAction } from 'svelte/attachments';
	import { longpress } from '$lib/actions/longpress';
	import { isMobile } from '$lib/utils/environment';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { previewFeatures } from '$lib/stores/preview-features.svelte';
	import {
		Navbar,
		NavbarBackLink,
		BlockTitle,
		List,
		ListItem,
		Preloader,
		Searchbar,
		useTheme,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '../profiles/Avatar.svelte';
	import IconButton from '../IconButton.svelte';
	import TitleTruncatedListItem from '../TitleTruncatedListItem.svelte';
	import ContactActionsMenu from '$lib/components/contacts/ContactActionsMenu.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const contacts = useReactivePromise(
		contactsStore.profilesForUnblockedContacts,
	);
	const theme = $derived(useTheme());

	let menuFor = $state<{
		agentId: AgentId;
		name: string;
		anchor: HTMLElement;
	} | null>(null);

	const attachLongPress = createAttachmentKey();

	function openMenu(agentId: AgentId, profile: Profile, anchor: HTMLElement) {
		menuFor = { agentId, name: fullName(profile), anchor };
	}

	const isAddContact = $derived(
		page.url.pathname === '/new-message/add-contact',
	);

	const isNewGroup = $derived(page.url.pathname === '/new-group');

	let searchQuery = $state('');
</script>

<div class="new-message-panel">
	<Navbar title={m.newMessage()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink
				onClick={() => {
					if (page.state.sidebarPanel === 'new-message') {
						history.back();
					} else {
						goto('/');
					}
				}}
				data-testid="new-message-back"
			/>
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div
			class={theme === 'ios' ? 'mt-6 px-4' : 'ps-5 pe-10'}
			data-testid="new-message-search"
		>
			<Searchbar
				clearButton
				placeholder={m.filter()}
				value={searchQuery}
				onInput={e => {
					searchQuery = e.target.value;
				}}
				onClear={() => {
					searchQuery = '';
				}}
			/>
		</div>

		<List
			strongIos
			inset={isWideScreen.value || theme === 'ios'}
			class="mb-0 mt-4"
		>
			<ListItem
				link
				class={isNewGroup ? 'active' : ''}
				linkProps={{ href: '/new-group' }}
				title={m.newGroup()}
				chevron={false}
				data-testid="new-message-new-group"
			>
				{#snippet media()}
					<wa-icon src={wrapPathInSvg(mdiAccountMultiplePlus)}></wa-icon>
				{/snippet}
			</ListItem>
			<ListItem
				link
				class={isAddContact ? 'active' : ''}
				linkProps={{ href: '/new-message/add-contact' }}
				title={m.addContact()}
				chevron={false}
				data-testid="new-message-add-contact"
			>
				{#snippet media()}
					<wa-icon src={wrapPathInSvg(mdiAccountPlus)}></wa-icon>
				{/snippet}
			</ListItem>
		</List>

		<BlockTitle>{m.contacts()}</BlockTitle>

		{#await $contacts}
			<div
				class="column"
				style="height: 100%; align-items: center; justify-content: center"
			>
				<Preloader />
			</div>
		{:then contacts}
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				data-testid="new-message-contact-list"
			>
				{#if contacts.length === 0}
					<ListItem title={m.noContactsYet()} />
				{:else}
					{@const filteredContacts = contacts.filter(([_, profile]) =>
						profile.name.toLowerCase().includes(searchQuery.toLowerCase()),
					)}
					{#each filteredContacts as [actorId, profile]}
						<TitleTruncatedListItem
							link
							class="hover-scope"
							linkProps={{
								href: `/direct-chats/${actorId}`,
								...(isMobile && {
									[attachLongPress]: fromAction(longpress, () => ({
										onLongPress: e => {
											// currentTarget is already null when the press timer
											// fires, so walk up from the pressed element instead.
											const row = (e.target as Element).closest('li');
											if (row) openMenu(actorId, profile, row);
										},
									})),
								}),
							}}
							title={profile.name}
							chevron={false}
						>
							{#snippet media()}
								<Avatar
									image={profile.avatar}
									initials={profile.name.slice(0, 2)}
								/>
							{/snippet}
							{#snippet after()}
								{#if !isMobile}
									<span class="hover-reveal">
										<IconButton
											icon={mdiDotsVertical}
											label={m.contactMenu()}
											testid="contact-menu-button"
											onClick={e => openMenu(actorId, profile, e.currentTarget)}
										/>
									</span>
								{/if}
							{/snippet}
						</TitleTruncatedListItem>
					{:else}
						<ListItem title={m.noContactsMatchFilter()} />
					{/each}
				{/if}
			</List>
		{/await}
	</div>
</div>

{#if menuFor}
	<ContactActionsMenu
		anchor={menuFor.anchor}
		agentId={menuFor.agentId}
		name={menuFor.name}
		onClose={() => (menuFor = null)}
	/>
{/if}

<style>
	.new-message-panel {
		display: flex;
		flex-direction: column;
	}
</style>
