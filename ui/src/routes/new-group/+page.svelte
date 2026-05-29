<script lang="ts">
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore, PublicKey } from 'dash-chat-stores';
	import MembersStep from './MembersStep.svelte';
	import DisabledGroupInfoStep from './DisabledGroupInfoStep.svelte';
	// To re-enable the real members step: swap DisabledMembersStep back to MembersStep

	const chatsStore: ChatsStore = getContext('chats-store');

	let currentPage: 'members' | 'group-info' = $state('members');
	let selectedContacts = $state<PublicKey[]>([]);

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
	<DisabledGroupInfoStep
		onBack={() => (currentPage = 'members')}
		onCreate={createGroupChat}
	/>
{/if}
