<script lang="ts">
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import type { Message } from 'dash-chat-stores';

	interface Props {
		/** Called with the message once the user agrees to drop the draft. */
		onConfirm: (message: Message) => void;
	}

	let { onConfirm }: Props = $props();

	let pending = $state<Message | null>(null);

	/** Ask the user to discard the current draft before editing `message`. */
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
	title={m.discardDraftTitle()}
	data-testid="composer-discard-draft-dialog"
>
	<span>{m.discardDraftDescription()}</span>
	{#snippet buttons()}
		<DialogButton
			data-testid="composer-discard-draft-cancel"
			onClick={() => (pending = null)}
		>
			{m.cancel()}
		</DialogButton>
		<DialogButton data-testid="composer-discard-draft-confirm" onClick={accept}>
			{m.discard()}
		</DialogButton>
	{/snippet}
</Dialog>
