<script lang="ts">
	import { getContext, type Snippet } from 'svelte';
	import type { BlobStore, FileAttachment } from 'dash-chat-stores';
	import {
		formatFileSize,
		mediaSize,
		saveFileAttachment,
		BlobLoadError,
	} from '$lib/utils/media';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import BlobProgressRing from '$lib/components/BlobProgressRing.svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import { Preloader } from 'konsta/svelte';

	interface Props {
		file: FileAttachment;
		/** Timestamp / receipts rendered inline at the end of the row, Signal-style
		 * (only on a file-only message; a captioned file shows them below). */
		metadata?: Snippet;
	}

	let { file, metadata }: Props = $props();

	const blobStore: BlobStore = getContext('blob-store');
	const download = $derived(useReactiveValue(blobStore.progress, file.hash));
	const complete = $derived($download?.complete === true);
	const stalled = $derived($download?.stalled === true);
	const bytes = $derived($download?.bytes ?? 0);

	let downloading = $state(false);

	async function handleSave() {
		if (!complete) {
			if (stalled) void blobStore.retry(file.hash);
			return;
		}
		if (downloading) return;
		downloading = true;
		try {
			if (await saveFileAttachment(file)) showToast(m.fileSaved());
		} catch (e) {
			if (e instanceof BlobLoadError)
				showToast(m.fileDownloadFailed(), 'error');
			else showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		} finally {
			downloading = false;
		}
	}
</script>

<button
	type="button"
	class="flex w-full cursor-pointer items-center border-none bg-transparent px-1 py-0.5 text-start text-inherit"
	data-testid="message-attachment-file"
	data-downloading={!complete}
	onclick={handleSave}
>
	<div
		class="me-2.5 flex h-10 w-8 shrink-0 items-center justify-center"
		data-testid="message-attachment-file-icon"
	>
		{#if !complete}
			<BlobProgressRing {bytes} total={mediaSize(file)} {stalled} size={32} />
		{:else if downloading}
			<Preloader class="h-6 w-6" />
		{:else}
			<ExtensionSheet name={file.name} />
		{/if}
	</div>
	<div class="flex min-w-0 flex-1 flex-col gap-px">
		<span
			class="overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap"
			>{file.name}</span
		>
		<span class="text-xs opacity-70" data-testid="message-attachment-file-size">
			{#if complete}
				{formatFileSize(mediaSize(file))}
			{:else}
				{formatFileSize(bytes)} / {formatFileSize(mediaSize(file))}
			{/if}
		</span>
	</div>
	{#if metadata}
		<div
			class="ms-2 flex shrink-0 items-center gap-1 self-end whitespace-nowrap select-none"
		>
			{@render metadata()}
		</div>
	{/if}
</button>
