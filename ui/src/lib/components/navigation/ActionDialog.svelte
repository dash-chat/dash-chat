<script lang="ts">
	import { Dialog, DialogButton, List, Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';

	type ActionResult = { success: true } | { success: false; error: string };

	type Props = {
		opened: boolean;
		onCancel: () => void;
		onConfirm: () => Promise<ActionResult>;
		title: string;
		cancelText?: string;
		confirmText: string;
		confirmTestId?: string;
	};

	let {
		opened,
		onCancel,
		onConfirm,
		title,
		cancelText = m.cancel(),
		confirmText,
		confirmTestId,
	}: Props = $props();

	let loading = $state(false);

	async function handleConfirm() {
		loading = true;
		try {
			const result = await onConfirm();
			if (!result.success) {
				showToast(result.error, 'error');
			}
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
		<DialogButton
			strong
			onClick={handleConfirm}
			disabled={loading}
			data-testid={confirmTestId}
		>
			{confirmText}
			{#if loading}
				<Preloader class="w-4 h-4 ml-2" />
			{/if}
		</DialogButton>
	{/snippet}
</Dialog>
