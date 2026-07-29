<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Dialog, DialogButton } from 'konsta/svelte';

	let {
		opened,
		name,
		blocked,
		onConfirm,
		onClose,
	}: {
		opened: boolean;
		name: string;
		blocked: boolean;
		onConfirm: () => void;
		onClose: () => void;
	} = $props();
</script>

<Dialog
	{opened}
	onBackdropClick={onClose}
	title={blocked
		? m.unblockContactTitle({ name })
		: m.blockContactTitle({ name })}
>
	<span>
		{blocked ? m.unblockContactDescription() : m.blockContactDescription()}
	</span>
	{#snippet buttons()}
		<DialogButton onClick={onClose}>{m.cancel()}</DialogButton>
		<DialogButton data-testid="block-contact-confirm" onClick={onConfirm}>
			{blocked ? m.unblock() : m.block()}
		</DialogButton>
	{/snippet}
</Dialog>
