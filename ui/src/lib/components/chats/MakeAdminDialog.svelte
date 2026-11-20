<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ChatsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';

	let {
		opened = $bindable(),
		chatId,
		actorId,
		memberName,
	}: {
		opened: boolean;
		chatId: string;
		actorId: string | null;
		memberName: string | undefined;
	} = $props();

	const chatsStore: ChatsStore = getContext('chats-store');

	async function confirm() {
		if (actorId === null) {
			return { success: false as const, error: m.errorUnexpected() };
		}
		try {
			await chatsStore
				.groupChats(chatId)
				.client.promoteToAdministrator(chatId, actorId);
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
	title={m.makeAdmin()}
	description={m.memberWillBeAbleToEditGroup({
		name: memberName || m.someone(),
	})}
	actions={[
		{
			text: m.makeAdmin(),
			testid: 'group-info-make-admin-confirm',
			onClick: confirm,
		},
	]}
/>
