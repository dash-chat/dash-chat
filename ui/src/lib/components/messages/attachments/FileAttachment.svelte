<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { FileAttachment } from 'dash-chat-stores';
	import { formatFileSize, mediaSize, saveFileAttachment } from '$lib/utils/media';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';

	interface Props {
		file: FileAttachment;
		/** Timestamp / receipts rendered inline at the end of the row, Signal-style
		 * (only on a file-only message; a captioned file shows them below). */
		metadata?: Snippet;
	}

	let { file, metadata }: Props = $props();

	async function handleSave() {
		try {
			if (await saveFileAttachment(file)) showToast(m.fileSaved());
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		}
	}
</script>

<button
	type="button"
	class="flex w-full cursor-pointer items-center border-none bg-transparent px-1 py-0.5 text-start text-inherit"
	data-testid="message-attachment-file"
	onclick={handleSave}
>
	<div class="me-2.5 flex shrink-0 items-center justify-center">
		<ExtensionSheet name={file.name} />
	</div>
	<div class="flex min-w-0 flex-1 flex-col gap-px">
		<span
			class="overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap"
			>{file.name}</span
		>
		<span class="text-xs opacity-70">{formatFileSize(mediaSize(file))}</span>
	</div>
	{#if metadata}
		<div
			class="ms-2 flex shrink-0 items-center gap-1 self-end whitespace-nowrap select-none"
		>
			{@render metadata()}
		</div>
	{/if}
</button>
