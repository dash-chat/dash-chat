<script lang="ts">
	import { onMount } from 'svelte';
	import {
		Button,
		Checkbox,
		Dialog,
		DialogButton,
		List,
		ListItem,
		Toast,
	} from 'konsta/svelte';
	import { mdiClose } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { sendMailto } from '$lib/utils/mailto';
	import { showToast, TOAST_TTL_MS, type ToastEvent } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages.js';

	let toastOpen = $state(false);
	let toastMessage = $state('');
	let toastVariant = $state<'default' | 'error' | 'unexpected'>('default');
	let toastTimeout: ReturnType<typeof setTimeout> | undefined;

	let errorReportDialogOpen = $state(false);
	let errorReportMessage = $state('');
	let errorReportError = $state<unknown>(undefined);
	let includeDebugLog = $state(true);

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

	function formatError(error: unknown): string {
		if (error instanceof Error) return error.message;
		if (typeof error === 'string') return error;
		try {
			return JSON.stringify(error);
		} catch {
			return String(error);
		}
	}

	function handleSendErrorReport() {
		toastOpen = false;
		clearTimeout(toastTimeout);
		errorReportMessage = toastMessage;
		includeDebugLog = true;
		errorReportDialogOpen = true;
	}

	async function sendErrorReport() {
		errorReportDialogOpen = false;

		const body = errorReportError
			? `${errorReportMessage}\n\nError: ${formatError(errorReportError)}`
			: errorReportMessage;

		try {
			await sendMailto({
				subject: 'Dash Chat: Error Report',
				body,
				includeDebugLog,
			});
		} catch {
			showToast(m.errorSendErrorReport(), 'error');
		}
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
	{toastMessage}
	{#snippet button()}
		{#if toastVariant === 'unexpected'}
			<Button inline clear onClick={handleSendErrorReport}>
				{m.sendErrorReport()}
			</Button>
			<button class="ml-1 opacity-70 active:opacity-100" onclick={dismissToast}>
				<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 18px"
				></wa-icon>
			</button>
		{/if}
	{/snippet}
</Toast>

<Dialog
	opened={errorReportDialogOpen}
	onBackdropClick={() => (errorReportDialogOpen = false)}
	title={m.sendErrorReport()}
>
	<p class="px-4 text-sm opacity-60">{m.errorReportExplanation()}</p>
	<List nested class="!my-0">
		<ListItem
			title={m.includeDebugLog()}
			onClick={() => (includeDebugLog = !includeDebugLog)}
		>
			{#snippet media()}
				<Checkbox
					checked={includeDebugLog}
					onChange={() => (includeDebugLog = !includeDebugLog)}
				/>
			{/snippet}
		</ListItem>
	</List>
	{#snippet buttons()}
		<DialogButton onClick={() => (errorReportDialogOpen = false)}>
			{m.cancel()}
		</DialogButton>
		<DialogButton strong onClick={sendErrorReport}>
			{m.send()}
		</DialogButton>
	{/snippet}
</Dialog>
