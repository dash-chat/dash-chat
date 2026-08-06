<script lang="ts">
	import { DialogButton } from 'konsta/svelte';
	import LazyDialog from '$lib/components/LazyDialog.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import type { DeviceId, Message, MessagesStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { canDeleteMessageForEveryone } from './message-helpers';
	import { showToast } from '$lib/utils/toasts';

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

	async function deleteForEveryone() {
		opened = false;
		try {
			await store.deleteMessageForEveryone(message);
		} catch (e) {
			console.error('Failed to delete message', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	async function deleteForMe() {
		opened = false;
		try {
			await store.deleteMessageForMe(message);
		} catch (e) {
			console.error('Failed to delete message for me', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

{#snippet cancelButton()}
	<DialogButton
		data-testid="delete-message-cancel"
		onClick={() => (opened = false)}
	>
		{m.cancel()}
	</DialogButton>
{/snippet}

{#snippet deleteForMeButton()}
	<DialogButton
		class="!text-red-500"
		data-testid="delete-message-for-me-confirm"
		onClick={deleteForMe}
	>
		{m.deleteForMe()}
	</DialogButton>
{/snippet}

<LazyDialog
	{opened}
	onBackdropClick={() => (opened = false)}
	title={m.deleteMessageTitle()}
	data-testid="delete-message-dialog"
>
	{#snippet buttons()}
		{#if forEveryone}
			<div class="flex flex-col w-full">
				<DialogButton
					class="!text-red-500"
					data-testid="delete-message-confirm"
					onClick={deleteForEveryone}
				>
					{m.deleteForEveryone()}
				</DialogButton>
				{@render deleteForMeButton()}
				{@render cancelButton()}
			</div>
		{:else}
			{@render cancelButton()}
			{@render deleteForMeButton()}
		{/if}
	{/snippet}
</LazyDialog>
