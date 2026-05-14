<script lang="ts">
	import {
		Checkbox,
		Dialog,
		DialogButton,
		List,
		ListItem,
	} from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { sendMailto } from '$lib/utils/mailto';
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

	function formatError(e: unknown): string {
		if (e instanceof Error) return e.stack ?? e.message;
		if (typeof e === 'string') return e;
		try {
			return JSON.stringify(e);
		} catch {
			return String(e);
		}
	}

	async function send() {
		opened = false;
		const body =
			error !== undefined
				? `${message}\n\nError: ${formatError(error)}`
				: message;
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
