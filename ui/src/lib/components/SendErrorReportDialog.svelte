<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import ActionDialog from '$lib/components/navigation/ActionDialog.svelte';
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
		try {
			await sendErrorReport({ message, error: describeError(error) });
			opened = false;
			showToast(m.reportSent());
			return { success: true as const };
		} catch {
			return { success: false as const, error: m.errorSendErrorReport() };
		}
	}
</script>

<ActionDialog
	{opened}
	onCancel={() => (opened = false)}
	title={m.sendErrorReport()}
	description={m.errorReportExplanation()}
	actions={[{ text: m.send(), onClick: send }]}
/>
