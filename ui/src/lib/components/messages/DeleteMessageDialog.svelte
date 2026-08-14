<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { DeviceId, Message, MessagesStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { canDeleteMessageForEveryone } from './message-helpers';
	import { lazyMount } from '$lib/stores/lazy-mount.svelte';

	interface Props {
		message: Message;
		myDeviceId: DeviceId;
		opened: boolean;
	}

	let { message, myDeviceId, opened = $bindable() }: Props = $props();

	const store: MessagesStore = getContext('messages-store');

	// Delete-for-me is offered on every message; delete-for-everyone only on my
	// own, and only within the delete window.
	const forEveryone = $derived(
		canDeleteMessageForEveryone(message, myDeviceId),
	);

	// There is one of these per message, so keep it out of the DOM until used.
	const prompt = lazyMount(() => opened);

	async function deleteForEveryone() {
		try {
			await store.deleteMessageForEveryone(message);
			opened = false;
			return { success: true as const };
		} catch (e) {
			console.error('Failed to delete message', e);
			return { success: false as const, error: m.errorUnexpected(), cause: e };
		}
	}

	async function deleteForMe() {
		try {
			await store.deleteMessageForMe(message);
			opened = false;
			return { success: true as const };
		} catch (e) {
			console.error('Failed to delete message for me', e);
			return { success: false as const, error: m.errorUnexpected(), cause: e };
		}
	}
</script>

{#if prompt.mounted}
	<ActionDialog
		opened={prompt.opened}
		onCancel={() => (opened = false)}
		title={m.deleteMessageTitle()}
		description={forEveryone ? m.deleteMessageDescription() : undefined}
		testid="delete-message-dialog"
		cancelTestId="delete-message-cancel"
		actions={[
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
		]}
	/>
{/if}
