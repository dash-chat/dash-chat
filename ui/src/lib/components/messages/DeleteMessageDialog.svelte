<script lang="ts">
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
	import { lazyMount } from '$lib/stores/lazy-mount.svelte';

	interface Props {
		opened: boolean;
		/** Called once the user confirms deleting the message for everyone. */
		onConfirm: () => void;
		onCancel: () => void;
	}

	let { opened, onConfirm, onCancel }: Props = $props();

	// There is one of these per outgoing message, so keep it out of the DOM until used.
	const prompt = lazyMount(() => opened);
</script>

{#if prompt.mounted}
	{#if isIos}
		<Actions
			opened={prompt.opened}
			onBackdropClick={onCancel}
			data-testid="delete-message-dialog"
		>
			<ActionsGroup class="flex flex-col gap-3 p-2.5">
				<div class="px-3.5 py-2 text-start text-xl text-black dark:text-white">
					{m.deleteMessageTitle()}
				</div>
				<ActionButton
					destructive
					onClick={onConfirm}
					data-testid="delete-message-confirm"
				>
					{m.deleteForEveryone()}
				</ActionButton>
				<ActionButton onClick={onCancel} data-testid="delete-message-cancel">
					{m.cancel()}
				</ActionButton>
			</ActionsGroup>
		</Actions>
	{:else}
		<Dialog
			opened={prompt.opened}
			onBackdropClick={onCancel}
			title={m.deleteMessageTitle()}
			data-testid="delete-message-dialog"
		>
			{#snippet buttons()}
				<DialogButton data-testid="delete-message-cancel" onClick={onCancel}>
					{m.cancel()}
				</DialogButton>
				<DialogButton data-testid="delete-message-confirm" onClick={onConfirm}>
					{m.deleteForEveryone()}
				</DialogButton>
			{/snippet}
		</Dialog>
	{/if}
{/if}
