<script lang="ts">
	import {
		Checkbox,
		Dialog,
		DialogButton,
		List,
		ListItem,
	} from 'konsta/svelte';
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

	let includeDebugLog = $state(true);

	async function send() {
		opened = false;
		try {
			await sendErrorReport({
				message,
				error: describeError(error),
				includeLog: includeDebugLog,
			});
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
		<DialogButton onClick={() => (opened = false)}>
			{m.cancel()}
		</DialogButton>
		<DialogButton strong onClick={send}>
			{m.send()}
		</DialogButton>
	{/snippet}
</Dialog>
