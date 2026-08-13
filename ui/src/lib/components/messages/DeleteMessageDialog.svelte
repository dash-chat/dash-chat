<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { DeviceId, Message, MessagesStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { canDeleteMessageForEveryone } from './message-helpers';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
	}

	let { message, myDeviceId }: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	// Delete-for-me is offered on every message; delete-for-everyone only on my
	// own, and only within the delete window.
	const forEveryone = $derived(
		canDeleteMessageForEveryone(message, myDeviceId),
	);

	async function deleteForEveryone() {
		dialog?.close();
		try {
			await store.deleteMessageForEveryone(message);
		} catch (e) {
			console.error('Failed to delete message', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	async function deleteForMe() {
		dialog?.close();
		try {
			await store.deleteMessageForMe(message);
		} catch (e) {
			console.error('Failed to delete message for me', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	const actions = $derived([
		...(forEveryone
			? [
					{
						text: m.deleteForEveryone(),
						destructive: true,
						testid: 'delete-message-confirm',
						onClick: deleteForEveryone,
					},
				]
			: []),
		{
			text: m.deleteForMe(),
			destructive: true,
			testid: 'delete-message-for-me-confirm',
			onClick: deleteForMe,
		},
	]);
</script>

<ActionDialog
	bind:this={dialog}
	title={m.deleteMessageTitle()}
	description={forEveryone ? m.deleteMessageDescription() : undefined}
	{actions}
	cancelTestId="delete-message-cancel"
	testid="delete-message-dialog"
/>
