<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { appLogDir, join } from '@tauri-apps/api/path';
	import { Button, Checkbox, Dialog, DialogButton, List, ListItem, Toast } from 'konsta/svelte';
	import { showToast, TOAST_TTL_MS, type ToastEvent } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages.js';

	let toastOpen = $state(false);
	let toastMessage = $state('');
	let toastVariant = $state<'default' | 'error' | 'unexpected'>('default');
	let toastTimeout: ReturnType<typeof setTimeout> | undefined;

	let errorReportDialogOpen = $state(false);
	let errorReportMessage = $state('');
	let includeDebugLog = $state(true);

	function handleToast(event: CustomEvent<ToastEvent>) {
		clearTimeout(toastTimeout);
		toastMessage = event.detail.message;
		toastVariant = event.detail.variant ?? 'default';
		toastOpen = true;
		toastTimeout = setTimeout(() => {
			toastOpen = false;
		}, TOAST_TTL_MS);
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

		let attachments: string[] | undefined;
		if (includeDebugLog) {
			const logDir = await appLogDir();
			const logFile = await join(logDir, 'Dash Chat.log');
			attachments = [logFile];
		}

		try {
			await invoke('plugin:mailto|mailto', {
				request: {
					email: 'hello@dashchat.org',
					subject: 'Dash Chat: Error Report',
					body: errorReportMessage,
					attachments,
				},
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
	class={toastVariant === 'error' || toastVariant === 'unexpected' ? 'k-color-brand-red' : ''}
	opened={toastOpen}
>
	{toastMessage}
	{#snippet button()}
		{#if toastVariant === 'unexpected'}
			<Button inline clear onClick={handleSendErrorReport}>
				{m.sendErrorReport()}
			</Button>
		{/if}
	{/snippet}
</Toast>

<Dialog
	opened={errorReportDialogOpen}
	onBackdropClick={() => (errorReportDialogOpen = false)}
>
	{#snippet title()}
		{m.sendErrorReport()}
	{/snippet}
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
