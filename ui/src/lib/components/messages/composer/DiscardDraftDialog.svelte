<script lang="ts">
	import { Actions, ActionsGroup, Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from '$lib/components/navigation/ActionButton.svelte';
	import ActionsTitle from '$lib/components/navigation/ActionsTitle.svelte';
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
		<ActionsGroup
			class="flex flex-col gap-2 !bg-white p-2.5 dark:!bg-neutral-900"
		>
			<ActionsTitle
				title={m.discardDraftTitle()}
				subtitle={m.discardDraftDescription()}
			/>
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
