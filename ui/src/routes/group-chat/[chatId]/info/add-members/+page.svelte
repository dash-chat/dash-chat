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
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');
	let selectedContacts = $state<VerifyingKey[]>([]);

	const contacts = useReactiveValue(contactsStore.profilesForAllContacts);
	const groupChatStore = chatsStore.groupChats(chatId);
	const members = useReactiveValue(groupChatStore.allMembers);

	const nonMemberContacts = $derived(
		($contacts ?? []).filter(([agentId]) => !($members && agentId in $members)),
	);

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
	constrainedWidth={true}
>
	<BlockTitle>{m.contacts()}</BlockTitle>

	<SelectableContactList
		contacts={nonMemberContacts}
		loading={$contacts === undefined || $members === undefined}
		noDataMessage={($contacts ?? []).length === 0
			? m.noContactsYet()
			: m.allContactsAlreadyInGroup()}
		bind:selectedContacts
	/>
</FormPage>
