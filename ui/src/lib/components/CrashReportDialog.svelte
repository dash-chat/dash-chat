<script lang="ts">
	import { onMount } from 'svelte';
	import { Dialog, DialogButton } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
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

	// Discarded on every way out, so a crash is offered once rather than at
	// every launch until the user acts on it.
	async function discard() {
		opened = false;
		await discardPendingCrashReport();
	}

	async function send() {
		opened = false;
		try {
			await sendPendingCrashReport();
			showToast(m.reportSent());
		} catch {
			showToast(m.errorSendErrorReport(), 'error');
		}
	}
</script>

<Dialog {opened} onBackdropClick={discard} title={m.appClosedUnexpectedly()}>
	<p class="text-sm opacity-60">{m.crashReportExplanation()}</p>
	{#snippet buttons()}
		<DialogButton data-testid="crash-report-discard" onClick={discard}>
			{m.dontSend()}
		</DialogButton>
		<DialogButton data-testid="crash-report-send" strong onClick={send}>
			{m.send()}
		</DialogButton>
	{/snippet}
</Dialog>
