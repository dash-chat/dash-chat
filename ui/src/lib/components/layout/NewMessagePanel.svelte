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
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
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
		Actions,
		ActionsGroup,
		ActionsButton,
		ActionsLabel,
		useTheme,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '../profiles/Avatar.svelte';
	import TitleTruncatedListItem from '../TitleTruncatedListItem.svelte';
	import BlockContactDialog from '$lib/components/contacts/BlockContactDialog.svelte';
	import ReportContactDialog from '$lib/components/contacts/ReportContactDialog.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);
	const blockedAgentIds = useReactiveValue(
		contactsStore.blockedContactAgentIds,
	);
	const theme = $derived(useTheme());

	let menuFor = $state<{ agentId: AgentId; profile: Profile } | null>(null);
	let menuIsBlocked = $state(false);
	let menuIsReported = $state(false);
	let showBlockDialog = $state(false);
	let showReportDialog = $state(false);
	let dialogFor = $state<{ agentId: AgentId; profile: Profile } | null>(null);

	async function openMenu(agentId: AgentId, profile: Profile, blocked: boolean) {
		menuFor = { agentId, profile };
		menuIsBlocked = blocked;
		menuIsReported = await contactsStore.client.isContactReported(agentId);
	}

	function requestBlockToggle() {
		if (!menuFor) return;
		dialogFor = menuFor;
		showBlockDialog = true;
		menuFor = null;
	}

	async function confirmBlockToggle() {
		if (!dialogFor) return;
		const { agentId } = dialogFor;
		showBlockDialog = false;
		if (menuIsBlocked) {
			await contactsStore.client.unblockContact(agentId);
		} else {
			await contactsStore.client.blockContact(agentId);
		}
		dialogFor = null;
	}

	function requestReport() {
		if (!menuFor) return;
		dialogFor = menuFor;
		showReportDialog = true;
		menuFor = null;
	}

	async function confirmReport() {
		if (!dialogFor) return;
		const { agentId } = dialogFor;
		showReportDialog = false;
		await contactsStore.reportContact(agentId);
		dialogFor = null;
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
			{@const blockedSet = $blockedAgentIds ?? new Set<AgentId>()}
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
						{@const blocked = blockedSet.has(actorId)}
						<TitleTruncatedListItem
							link
							linkProps={{ href: `/direct-chats/${actorId}` }}
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
								{#if blocked}
									<span class="quiet me-2 text-xs">{m.blocked()}</span>
								{/if}
								<button
									class="p-1"
									onclick={e => {
										e.preventDefault();
										e.stopPropagation();
										openMenu(actorId, profile, blocked);
									}}
									aria-label={m.block()}
									data-testid="contact-menu-button"
								>
									<wa-icon src={wrapPathInSvg(mdiDotsVertical)}></wa-icon>
								</button>
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

<Actions opened={menuFor !== null} onBackdropClick={() => (menuFor = null)}>
	<ActionsGroup>
		{#if menuFor}
			<ActionsLabel>{fullName(menuFor.profile)}</ActionsLabel>
		{/if}
		<ActionsButton
			bold
			onClick={requestBlockToggle}
			data-testid="contact-block-toggle"
		>
			{menuIsBlocked ? m.unblock() : m.block()}
		</ActionsButton>
		<ActionsButton
			onClick={menuIsReported ? () => (menuFor = null) : requestReport}
			data-testid="contact-report"
		>
			{menuIsReported ? m.reported() : m.report()}
		</ActionsButton>
	</ActionsGroup>
	<ActionsGroup>
		<ActionsButton onClick={() => (menuFor = null)}>
			{m.cancel()}
		</ActionsButton>
	</ActionsGroup>
</Actions>

{#if dialogFor}
	<BlockContactDialog
		opened={showBlockDialog}
		name={fullName(dialogFor.profile)}
		blocked={menuIsBlocked}
		onConfirm={confirmBlockToggle}
		onClose={() => {
			showBlockDialog = false;
			dialogFor = null;
		}}
	/>
	<ReportContactDialog
		opened={showReportDialog}
		name={fullName(dialogFor.profile)}
		onConfirm={confirmReport}
		onClose={() => {
			showReportDialog = false;
			dialogFor = null;
		}}
	/>
{/if}

<style>
	.new-message-panel {
		display: flex;
		flex-direction: column;
	}
</style>
