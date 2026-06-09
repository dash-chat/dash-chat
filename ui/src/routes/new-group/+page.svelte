<script lang="ts">
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type {
		ChatsStore,
		ContactsStore,
		VerifyingKey,
	} from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import MembersStep from './MembersStep.svelte';
	import GroupInfoStep from './GroupInfoStep.svelte';

	const chatsStore: ChatsStore = getContext('chats-store');
	const contactsStore: ContactsStore = getContext('contacts-store');
	const contacts = useReactiveValue(contactsStore.profilesForAllContacts);
	const resolvedContacts = $derived($contacts ?? []);

	let currentPage: 'members' | 'group-info' = $state('members');
	let selectedContacts = $state<VerifyingKey[]>([]);
	let groupName = $state('');

	async function createGroupChat() {
		const groupStore = await chatsStore.createGroup(
			Array.from(selectedContacts),
		);
		goto(`/group-chat/${groupStore.chatId}`);
	}
</script>

{#if currentPage === 'members'}
	<MembersStep
		bind:selectedContacts
		onNext={() => (currentPage = 'group-info')}
	/>
{:else if currentPage === 'group-info'}
	<GroupInfoStep
		bind:groupName
		{selectedContacts}
		{resolvedContacts}
		onBack={() => (currentPage = 'members')}
		onCreate={createGroupChat}
	/>
{/if}
