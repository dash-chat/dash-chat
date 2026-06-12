<script lang="ts">
	import { Dialog, DialogButton, List, Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';

	type Props = {
		opened: boolean;
		onCancel: () => void;
		onConfirm: () => Promise<void>;
		title: string;
		cancelText?: string;
		confirmText: string;
	};

	let {
		opened,
		onCancel,
		onConfirm,
		title,
		cancelText = m.cancel(),
		confirmText,
	}: Props = $props();

	let loading = $state(false);

	async function handleConfirm() {
		loading = true;
		try {
			await onConfirm();
		} finally {
			loading = false;
		}
	}
</script>

<Dialog {opened} onBackdropClick={onCancel} {title}>
	<span>{m.areYouSureLeaveGroup()}</span>
	{#snippet buttons()}
		<DialogButton onClick={onCancel} disabled={loading}>
			{cancelText}
		</DialogButton>
		<DialogButton strong onClick={handleConfirm} disabled={loading}>
			{confirmText}
			{#if loading}
				<Preloader class="w-4 h-4 ml-2" />
			{/if}
		</DialogButton>
	{/snippet}
</Dialog>
