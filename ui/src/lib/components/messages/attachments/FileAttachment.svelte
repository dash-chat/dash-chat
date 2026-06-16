<script lang="ts">
	import type { FileAttachment } from 'dash-chat-stores';
	import { byteLengthOf, formatFileSize } from '$lib/types/media';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import { saveAttachment } from '$lib/utils/save-file';

	interface Props {
		file: FileAttachment;
	}

	let { file }: Props = $props();
</script>

<button
	type="button"
	class="attachment-file"
	data-testid="message-attachment-file"
	onclick={() => saveAttachment(file)}
>
	<div class="attachment-file-icon">
		<ExtensionSheet name={file.name} />
	</div>
	<div class="attachment-file-info">
		<span class="attachment-file-name">{file.name}</span>
		<span class="attachment-file-size"
			>{formatFileSize(byteLengthOf(file.data))}</span
		>
	</div>
</button>

<style>
	.attachment-file {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 2px 4px;
		border: none;
		background: transparent;
		cursor: pointer;
		text-align: start;
		color: inherit;
	}

	.attachment-file-icon {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-inline-end: 0.625rem;
	}

	.attachment-file-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.attachment-file-name {
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.attachment-file-size {
		font-size: 12px;
		opacity: 0.7;
	}
</style>
