<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import {
		discardPendingCrashReport,
		hasPendingCrashReport,
		sendPendingCrashReport,
	} from '$lib/utils/error-report';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import { showToast } from '$lib/utils/toasts';

	let dialog = $state<ActionDialog>();

	onMount(async () => {
		if (await hasPendingCrashReport()) dialog?.show();
	});

	async function send() {
		dialog?.close();
		try {
			await sendPendingCrashReport();
			showToast(m.reportSent());
		} catch {
			showToast(m.errorSendErrorReport(), 'error');
		}
	}
</script>

<ActionDialog
	bind:this={dialog}
	title={m.appClosedUnexpectedly()}
	description={m.crashReportExplanation()}
	actions={[
		{
			text: m.send(),
			strong: true,
			testid: 'crash-report-send',
			onClick: send,
		},
	]}
	cancelText={m.dontSend()}
	cancelTestId="crash-report-discard"
	onCancel={() => void discardPendingCrashReport()}
/>
