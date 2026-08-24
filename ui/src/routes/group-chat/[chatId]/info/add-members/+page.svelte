<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type {
		ChatsStore,
		ContactsStore,
		VerifyingKey,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { BlockTitle } from 'konsta/svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';
	import FormPage from '$lib/components/layout/FormPage.svelte';
	import { page } from '$app/state';
	import ContactSearchNav from '$lib/components/contacts/ContactSearchNav.svelte';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');
	let selectedContacts = $state<VerifyingKey[]>([]);

	const contacts = useReactiveValue(contactsStore.profilesForUnblockedContacts);
	const groupChatStore = chatsStore.groupChats(chatId);
	const members = useReactiveValue(groupChatStore.allMembers);
	const loading = $derived($contacts === undefined || $members === undefined);

	const nonMemberContacts = $derived(
		($contacts ?? []).filter(
			({ contact }) => !($members && contact.agentId in $members),
		),
	);
	let filteredContacts = $state<typeof nonMemberContacts>([]);
	let searchQuery = $state('');

	const noDataMessage = $derived.by(() => {
		if (loading) return '';
		if (($contacts ?? []).length === 0) return m.noContactsYet();
		if (filteredContacts.length === 0 && searchQuery.length > 0)
			return m.noContactsMatchFilter();
		if (filteredContacts.length === 0) return m.allContactsAlreadyInGroup();
		return '';
	});

	async function addMembers() {
		const store = chatsStore.groupChats(chatId);
		await store.addMembers(selectedContacts);
		goto(`/group-chat/${chatId}/info`);
	}
</script>

<FormPage
	title={m.addMembers()}
	actionLabel={m.add()}
	onAction={addMembers}
	actionDisabled={selectedContacts.length === 0}
	onBack={() => goto(`/group-chat/${chatId}/info`)}
	constrainedWidth
	backTestId="add-members-back"
	actionTestId="add-members-add-btn"
>
	{#snippet subnavbar()}
		<ContactSearchNav
			bind:searchQuery
			bind:filteredContacts
			{selectedContacts}
			contacts={nonMemberContacts}
			onRemove={key => {
				selectedContacts = selectedContacts.filter(c => c !== key);
			}}
		/>
	{/snippet}

	<BlockTitle>{m.contacts()}</BlockTitle>

	<SelectableContactList
		contacts={filteredContacts}
		{loading}
		{noDataMessage}
		bind:selectedContacts
	/>
</FormPage>
