<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Toast } from 'konsta/svelte';
	import { mdiClose } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TOAST_TTL_MS, type ToastEvent } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages.js';
	import SendErrorReportDialog from '$lib/components/SendErrorReportDialog.svelte';

	let toastOpen = $state(false);
	let toastMessage = $state('');
	let toastVariant = $state<'default' | 'error' | 'unexpected'>('default');
	let toastTimeout: ReturnType<typeof setTimeout> | undefined;

	let errorReportDialogOpen = $state(false);
	let errorReportMessage = $state('');
	let errorReportError = $state<unknown>(undefined);

	function handleToast(event: CustomEvent<ToastEvent>) {
		clearTimeout(toastTimeout);
		toastMessage = event.detail.message;
		toastVariant = event.detail.variant ?? 'default';
		toastOpen = true;
		if (event.detail.error !== undefined) {
			errorReportError = event.detail.error;
		}
		if (toastVariant !== 'unexpected') {
			toastTimeout = setTimeout(() => {
				toastOpen = false;
			}, TOAST_TTL_MS);
		}
	}

	function dismissToast() {
		toastOpen = false;
		clearTimeout(toastTimeout);
	}

	function handleSendErrorReport() {
		toastOpen = false;
		clearTimeout(toastTimeout);
		errorReportMessage = toastMessage;
		errorReportDialogOpen = true;
	}

	onMount(() => {
		window.addEventListener('app:toast', handleToast as EventListener);
		return () => {
			window.removeEventListener('app:toast', handleToast as EventListener);
			clearTimeout(toastTimeout);
		};
	});
</script>

<Toast
	style={toastVariant === 'unexpected' ? '' : 'pointer-events: none'}
	position="center"
	class={toastVariant === 'error' || toastVariant === 'unexpected'
		? 'k-color-brand-red'
		: ''}
	opened={toastOpen}
>
	{#if toastOpen}
		<span data-testid="toast">{toastMessage}</span>
	{/if}
	{#snippet button()}
		{#if toastVariant === 'unexpected'}
			<Button inline clear onClick={handleSendErrorReport}>
				{m.sendErrorReport()}
			</Button>
			<button
				class="ms-1 opacity-70 active:opacity-100"
				onclick={dismissToast}
				aria-label={m.close()}
			>
				<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 18px"
				></wa-icon>
			</button>
		{/if}
	{/snippet}
</Toast>

<SendErrorReportDialog
	bind:opened={errorReportDialogOpen}
	message={errorReportMessage}
	error={errorReportError}
/>
