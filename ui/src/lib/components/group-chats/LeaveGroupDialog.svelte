<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatId, ChatsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let { chatId }: { chatId: ChatId } = $props();

	const chatsStore: ChatsStore = getContext('chats-store');

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function confirm() {
		try {
			await chatsStore.leaveGroup(chatId);
			goto('/');
			dialog?.close();
		} catch (e) {
			console.error(e);
			const errorMessage =
				(e as { kind?: string }).kind === 'LastAdmin'
					? m.errorLeavingGroupOnlyAdmin()
					: m.errorLeavingGroup();
			showToast(errorMessage, 'error');
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
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
