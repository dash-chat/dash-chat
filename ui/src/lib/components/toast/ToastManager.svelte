<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Toast } from 'konsta/svelte';
	import { mdiClose } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { TOAST_TTL_MS, type ToastEvent } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages.js';
	import SendErrorReportDialog from '$lib/components/SendErrorReportDialog.svelte';
	import { keyboard } from 'tauri-plugin-virtual-keyboard';
	import { useSignal } from '$lib/stores/use-signal';

	const keyboardHeight = useSignal(() => keyboard.height.value);

	let toastOpen = $state(false);
	let toastMessage = $state('');
	let toastVariant = $state<'default' | 'error' | 'unexpected'>('default');
	let toastTimeout: ReturnType<typeof setTimeout> | undefined;

	// Expose the most recent toast event for e2e diagnostics.
	(
		window as Window & {
			__lastToastEvent?: { message: string; variant: string };
		}
	).__lastToastEvent = undefined;

	let errorReportDialogOpen = $state(false);
	let errorReportMessage = $state('');
	let errorReportError = $state<unknown>(undefined);

	function handleToast(event: CustomEvent<ToastEvent>) {
		clearTimeout(toastTimeout);
		toastMessage = event.detail.message;
		toastVariant = event.detail.variant ?? 'default';
		(
			window as Window & {
				__lastToastEvent?: { message: string; variant: string };
			}
		).__lastToastEvent = { message: toastMessage, variant: toastVariant };
		toastOpen = true;
		if (event.detail.error !== undefined) {
			errorReportError = event.detail.error;
		}
		// Without Sentry there is no report action to wait for, so even
		// unexpected toasts auto-hide.
		if (toastVariant !== 'unexpected' || !import.meta.env.VITE_SENTRY_ENABLED) {
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

<!-- Konsta's toast root is a full-width bar pinned across the bottom of the
	screen with the pill centred inside it, so leaving it hit-testable covers
	whatever sits in the bottom corners (the new-message FAB). Only the controls
	take pointer events. -->
<Toast
	style="pointer-events: none; --keyboard-visible-height: {$keyboardHeight}px"
	position="center"
	class={toastVariant === 'error' || toastVariant === 'unexpected'
		? 'k-color-brand-red'
		: ''}
	opened={toastOpen}
>
	<span data-testid="toast">{toastMessage}</span>
	{#snippet button()}
		{#if toastVariant === 'unexpected' && import.meta.env.VITE_SENTRY_ENABLED}
			<div class="pointer-events-auto">
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
			</div>
		{/if}
	{/snippet}
</Toast>

<SendErrorReportDialog
	bind:opened={errorReportDialogOpen}
	message={errorReportMessage}
	error={errorReportError}
/>
