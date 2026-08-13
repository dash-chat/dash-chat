<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import type { Message } from 'dash-chat-stores';

	interface Props {
		/** Called with the message once the user agrees to drop the draft. */
		onConfirm: (message: Message) => void;
	}

	let { onConfirm }: Props = $props();

	let dialog = $state<ActionDialog>();
	let pending: Message | null = null;

	/** Ask the user to discard the current draft before editing `message`. */
	export function confirm(message: Message) {
		pending = message;
		dialog?.show();
	}

	function accept() {
		const message = pending;
		pending = null;
		dialog?.close();
		if (message) onConfirm(message);
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.discardDraftTitle()}
	description={m.discardDraftDescription()}
	actions={[
		{
			text: m.discard(),
			destructive: true,
			testid: 'composer-discard-draft-confirm',
			onClick: accept,
		},
	]}
	cancelTestId="composer-discard-draft-cancel"
	onCancel={() => (pending = null)}
	testid="composer-discard-draft-dialog"
/>
