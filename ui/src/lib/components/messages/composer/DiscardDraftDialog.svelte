<script lang="ts">
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
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

{#if isIos}
	<Actions
		opened={pending !== null}
		onBackdropClick={() => (pending = null)}
		data-testid="composer-discard-draft-dialog"
	>
		<ActionsGroup class="flex flex-col gap-3 p-2.5">
			<div class="flex flex-col gap-1 px-3.5 py-2 text-start">
				<span class="text-xl text-black dark:text-white">
					{m.discardDraftTitle()}
				</span>
				<span class="text-black/60 dark:text-white/60">
					{m.discardDraftDescription()}
				</span>
			</div>
			<ActionButton
				destructive
				onClick={accept}
				data-testid="composer-discard-draft-confirm"
			>
				{m.discard()}
			</ActionButton>
			<ActionButton
				onClick={() => (pending = null)}
				data-testid="composer-discard-draft-cancel"
			>
				{m.cancel()}
			</ActionButton>
		</ActionsGroup>
	</Actions>
{:else}
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
			<DialogButton
				data-testid="composer-discard-draft-confirm"
				onClick={accept}
			>
				{m.discard()}
			</DialogButton>
		{/snippet}
	</Dialog>
{/if}
