<script lang="ts">
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import type { Message } from 'dash-chat-stores';

	interface Props {
		/** Called with the message once the user confirms deleting it for everyone. */
		onConfirm: (message: Message) => void;
	}

	let { onConfirm }: Props = $props();

	let pending = $state<Message | null>(null);

	/** Ask the user to confirm deleting `message` for everyone. */
	export function confirm(message: Message) {
		pending = message;
	}

	function accept() {
		const message = pending;
		pending = null;
		if (message) onConfirm(message);
	}
</script>

<Dialog
	opened={pending !== null}
	onBackdropClick={() => (pending = null)}
	title={m.deleteMessageTitle()}
	data-testid="composer-delete-message-dialog"
>
	{#snippet buttons()}
		<DialogButton
			data-testid="composer-delete-cancel"
			onClick={() => (pending = null)}
		>
			{m.cancel()}
		</DialogButton>
		<DialogButton data-testid="composer-delete-confirm" onClick={accept}>
			{m.deleteForEveryone()}
		</DialogButton>
	{/snippet}
</Dialog>
