<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ChatsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';

	let {
		opened = $bindable(),
		chatId,
		actorId,
	}: {
		opened: boolean;
		chatId: string;
		actorId: string | null;
	} = $props();

	const chatsStore: ChatsStore = getContext('chats-store');

	async function confirm() {
		if (actorId === null) {
			return { success: false as const, error: m.errorUnexpected() };
		}
		try {
			await chatsStore.groupChats(chatId).client.removeMember(chatId, actorId);
			opened = false;
			return { success: true as const };
		} catch (e) {
			console.error(e);
			return { success: false as const, error: m.errorUnexpected(), cause: e };
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.removeMember()}
	description={m.areYouSureRemoveMember()}
	actions={[
		{
			text: m.remove(),
			destructive: true,
			testid: 'group-info-remove-member-confirm',
			onClick: confirm,
		},
	]}
/>
