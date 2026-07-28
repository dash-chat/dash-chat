<script lang="ts">
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		opened: boolean;
		/** Called once the user confirms deleting the message for everyone. */
		onConfirm: () => void;
		onCancel: () => void;
	}

	let { opened, onConfirm, onCancel }: Props = $props();
</script>

<Dialog
	{opened}
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
