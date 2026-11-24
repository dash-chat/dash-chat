<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
	import {
		discardPendingCrashReport,
		hasPendingCrashReport,
		sendPendingCrashReport,
	} from '$lib/utils/error-report';
	import { showToast } from '$lib/utils/toasts';

	let opened = $state(false);

	onMount(async () => {
		opened = await hasPendingCrashReport();
	});

	async function discard() {
		opened = false;
		await discardPendingCrashReport();
	}

	async function send() {
		try {
			const outcome = await sendPendingCrashReport();
			opened = false;
			showToast(outcome === 'queued' ? m.reportQueued() : m.reportSent());
			return { success: true as const };
		} catch {
			return { success: false as const, error: m.errorSendErrorReport() };
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={discard}
	title={m.appClosedUnexpectedly()}
	description={m.crashReportExplanation()}
	cancelText={m.dontSend()}
	cancelTestId="crash-report-discard"
	actions={[{ text: m.send(), testid: 'crash-report-send', onClick: send }]}
/>
