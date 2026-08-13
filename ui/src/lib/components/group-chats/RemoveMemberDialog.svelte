<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { AgentId, ChatId, ChatsStore } from 'dash-chat-stores';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let { chatId }: { chatId: ChatId } = $props();

	const chatsStore: ChatsStore = getContext('chats-store');

	let dialog = $state<ActionDialog>();
	let pending: AgentId | null = null;

	/** Ask the user to confirm removing `actorId` from the group. */
	export function show(actorId: AgentId) {
		pending = actorId;
		dialog?.show();
	}

	async function confirm() {
		const actorId = pending;
		if (!actorId) return;
		try {
			await chatsStore.groupChats(chatId).client.removeMember(chatId, actorId);
			pending = null;
			dialog?.close();
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'error');
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
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
	onCancel={() => (pending = null)}
/>
