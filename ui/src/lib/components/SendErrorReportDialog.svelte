<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { describeError, sendErrorReport } from '$lib/utils/error-report';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let {
		message,
		error,
	}: {
		message: string;
		error?: unknown;
	} = $props();

	let dialog = $state<ActionDialog>();

	export function show() {
		dialog?.show();
	}

	async function send() {
		dialog?.close();
		try {
			await sendErrorReport({ message, error: describeError(error) });
			showToast(m.reportSent());
		} catch {
			showToast(m.errorSendErrorReport(), 'error');
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.sendErrorReport()}
	description={m.errorReportExplanation()}
	actions={[{ text: m.send(), strong: true, onClick: send }]}
/>
