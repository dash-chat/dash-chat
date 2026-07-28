<script lang="ts">
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	interface Props {
		/** Called once the user confirms deleting the message for everyone. */
		onConfirm: () => void;
		onCancel: () => void;
	}

	let { onConfirm, onCancel }: Props = $props();

	// This component is only rendered while the confirmation is up, so it has to
	// open a frame after mounting: a dialog mounted already-open never plays
	// Konsta's open transition.
	let opened = $state(false);
	$effect(() => {
		const frame = requestAnimationFrame(() => (opened = true));
		return () => cancelAnimationFrame(frame);
	});
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
