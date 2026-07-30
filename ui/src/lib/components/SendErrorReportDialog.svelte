<script lang="ts">
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { describeError, sendErrorReport } from '$lib/utils/error-report';
	import { showToast } from '$lib/utils/toasts';

	let {
		opened = $bindable(),
		message,
		error,
	}: {
		opened: boolean;
		message: string;
		error?: unknown;
	} = $props();

	async function send() {
		opened = false;
		try {
			await sendErrorReport({ message, error: describeError(error) });
			showToast(m.reportSent());
		} catch {
			showToast(m.errorSendErrorReport(), 'error');
		}
	}
</script>

<Dialog
	{opened}
	onBackdropClick={() => (opened = false)}
	title={m.sendErrorReport()}
>
	<p class="text-sm opacity-60">{m.errorReportExplanation()}</p>
	{#snippet buttons()}
		<DialogButton onClick={() => (opened = false)}>
			{m.cancel()}
		</DialogButton>
		<DialogButton strong onClick={send}>
			{m.send()}
		</DialogButton>
	{/snippet}
</Dialog>
