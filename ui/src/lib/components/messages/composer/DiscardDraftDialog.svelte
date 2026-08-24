<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
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

	async function accept() {
		const message = pending;
		pending = null;
		if (message) onConfirm(message);
		return { success: true as const };
	}
</script>

<ActionDialog
	opened={pending !== null}
	onCancel={() => (pending = null)}
	title={m.discardDraftTitle()}
	description={m.discardDraftDescription()}
	testid="composer-discard-draft-dialog"
	cancelTestId="composer-discard-draft-cancel"
	actions={[
		{
			text: m.discard(),
			destructive: true,
			testid: 'composer-discard-draft-confirm',
			onClick: accept,
		},
	]}
/>
