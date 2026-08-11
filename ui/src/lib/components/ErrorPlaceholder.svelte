<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Button } from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAlert } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import SendErrorReportDialog from './SendErrorReportDialog.svelte';

	let { message, error }: { message: string; error?: unknown } = $props();

	let dialogOpen = $state(false);
</script>

<div
	class="quiet flex flex-1 flex-col items-center justify-center pb-16 text-center gap-2"
>
	<wa-icon src={wrapPathInSvg(mdiAlert)} style="font-size: 3rem"></wa-icon>
	<div class="flex flex-col">
		<span>{message}</span>
		{#if error !== undefined}
			{#if import.meta.env.VITE_SENTRY_ENABLED}
				<Button inline clear onClick={() => (dialogOpen = true)}>
					{m.sendErrorReport()}
				</Button>
			{/if}
		{/if}
	</div>
</div>

<SendErrorReportDialog bind:opened={dialogOpen} {message} {error} />
