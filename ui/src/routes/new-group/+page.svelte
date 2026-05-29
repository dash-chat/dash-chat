<script lang="ts">
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore, PublicKey } from 'dash-chat-stores';
	import MembersStep from './MembersStep.svelte';
	import GroupInfoStep from './GroupInfoStep.svelte';

	const chatsStore: ChatsStore = getContext('chats-store');

	let currentPage: 'members' | 'group-info' = $state('members');
	let selectedContacts = $state<PublicKey[]>([]);
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
		onBack={() => (currentPage = 'members')}
		onCreate={createGroupChat}
	/>
{/if}
