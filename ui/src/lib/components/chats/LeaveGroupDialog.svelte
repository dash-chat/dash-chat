<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';

	let {
		opened = $bindable(),
		chatId,
	}: {
		opened: boolean;
		chatId: string;
	} = $props();

	const chatsStore: ChatsStore = getContext('chats-store');

	async function confirm() {
		try {
			await chatsStore.leaveGroup(chatId);
			goto('/');
			opened = false;
			return { success: true as const };
		} catch (e) {
			console.error(e);
			if ((e as { kind?: string }).kind === 'LastAdmin') {
				return {
					success: false as const,
					error: m.errorLeavingGroupOnlyAdmin(),
				};
			}
			return {
				success: false as const,
				error: m.errorLeavingGroup(),
				cause: e,
			};
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.leaveGroup()}
	description={m.areYouSureLeaveGroup()}
	actions={[
		{
			text: m.leave(),
			destructive: true,
			testid: 'group-info-leave-confirm',
			onClick: confirm,
		},
	]}
/>
