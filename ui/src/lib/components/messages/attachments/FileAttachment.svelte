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
	import { onScreen } from '$lib/utils/on-screen';
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
	let visible = $state(false);
	const hash = $derived(file.hash);
	const progress = $derived(
		visible ? useReactiveValue(blobStore.progress, hash) : undefined,
	);
	const downloading = $derived(
		$progress !== undefined && !$progress.complete ? $progress : undefined,
	);

	let saving = $state(false);

	// A tap on a file still downloading asks the node for it now and says so;
	// the save itself waits for a tap once the blob has landed.
	async function handleSave() {
		if (saving) return;
		if (downloading !== undefined) {
			blobStore.retry(hash);
			showToast(m.fileStillDownloading());
			return;
		}
		saving = true;
		try {
			if (await saveFileAttachment(file)) showToast(m.fileSaved());
		} catch (e) {
			if (e instanceof BlobLoadError)
				showToast(m.fileDownloadFailed(), 'error');
			else showToast(m.errorUnexpected(), 'unexpected', e);
			console.error(e);
		} finally {
			saving = false;
		}
	}
</script>

<button
	type="button"
	class="flex w-full cursor-pointer items-center border-none bg-transparent px-1 py-0.5 text-start text-inherit"
	data-testid="message-attachment-file"
	onclick={handleSave}
	{@attach onScreen(v => (visible = v))}
>
	<div
		class="me-2.5 flex h-10 w-8 shrink-0 items-center justify-center"
		data-testid="message-attachment-file-icon"
	>
		{#if downloading !== undefined}
			<BlobProgressRing
				bytes={downloading.bytes}
				total={mediaSize(file)}
				stalled={downloading.stalled}
				size={32}
			/>
		{:else if saving}
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
			{#if downloading !== undefined}
				{formatFileSize(downloading.bytes)} / {formatFileSize(mediaSize(file))}
			{:else}
				{formatFileSize(mediaSize(file))}
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
