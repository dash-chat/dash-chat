<script lang="ts">
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import type { DeviceId, Message, MessagesStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import ActionsTitle from '$lib/components/navigation/ActionsTitle.svelte';
	import { canDeleteMessageForEveryone } from './message-helpers';
	import { isIos } from '$lib/utils/environment';
	import { lazyMount } from '$lib/stores/lazy-mount.svelte';
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

	// There is one of these per message, so keep it out of the DOM until used.
	const prompt = lazyMount(() => opened);

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

{#if prompt.mounted}
	{#if isIos}
		<Actions
			opened={prompt.opened}
			onBackdropClick={() => (opened = false)}
			data-testid="delete-message-dialog"
		>
			<ActionsGroup
				class="flex flex-col gap-2 !bg-white p-2.5 dark:!bg-neutral-900"
			>
				<ActionsTitle
					title={m.deleteMessageTitle()}
					subtitle={forEveryone ? m.deleteMessageDescription() : undefined}
				/>
				{#if forEveryone}
					<ActionButton
						destructive
						onClick={deleteForEveryone}
						data-testid="delete-message-confirm"
					>
						{m.deleteForEveryone()}
					</ActionButton>
				{/if}
				<ActionButton
					destructive
					onClick={deleteForMe}
					data-testid="delete-message-for-me-confirm"
				>
					{m.deleteForMe()}
				</ActionButton>
				<ActionButton
					onClick={() => (opened = false)}
					data-testid="delete-message-cancel"
				>
					{m.cancel()}
				</ActionButton>
			</ActionsGroup>
		</Actions>
	{:else}
		<Dialog
			opened={prompt.opened}
			onBackdropClick={() => (opened = false)}
			title={m.deleteMessageTitle()}
			data-testid="delete-message-dialog"
		>
			{#if forEveryone}
				{m.deleteMessageDescription()}
			{/if}
			{#snippet buttons()}
				{#if forEveryone}
					<div class="flex w-full flex-col items-end gap-2">
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
		</Dialog>
	{/if}
{/if}
