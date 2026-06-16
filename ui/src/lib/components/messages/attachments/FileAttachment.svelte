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
	class="flex w-full cursor-pointer items-center border-none bg-transparent px-1 py-0.5 text-start text-inherit"
	data-testid="message-attachment-file"
	onclick={() => saveAttachment(file)}
>
	<div class="me-2.5 flex shrink-0 items-center justify-center">
		<ExtensionSheet name={file.name} />
	</div>
	<div class="flex min-w-0 flex-1 flex-col gap-px">
		<span
			class="overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap"
			>{file.name}</span
		>
		<span class="text-xs opacity-70"
			>{formatFileSize(byteLengthOf(file.data))}</span
		>
	</div>
</button>
