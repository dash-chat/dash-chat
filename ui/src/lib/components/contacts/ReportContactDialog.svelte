<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { Dialog, DialogButton } from 'konsta/svelte';

	let {
		opened,
		name,
		onConfirm,
		onClose,
	}: {
		opened: boolean;
		name: string;
		onConfirm: () => void;
		onClose: () => void;
	} = $props();
</script>

<Dialog {opened} onBackdropClick={onClose} title={m.reportContactTitle({ name })}>
	<span>{m.reportContactDescription()}</span>
	{#snippet buttons()}
		<DialogButton onClick={onClose}>{m.cancel()}</DialogButton>
		<DialogButton data-testid="report-contact-confirm" onClick={onConfirm}>
			{m.report()}
		</DialogButton>
	{/snippet}
</Dialog>
