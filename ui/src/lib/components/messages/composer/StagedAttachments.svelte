<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiPlus } from '@mdi/js';
	import {
		type DraftMedia,
		MAX_STAGED_PHOTOS,
		PHOTO_ACCEPT,
	} from '$lib/utils/media';
	import { objectUrl } from '$lib/actions/object-url';
	import { pickFiles } from '$lib/utils/files';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import StagedThumb from './StagedThumb.svelte';

	interface Props {
		media: DraftMedia | undefined;
		/** Stage files picked from the "add more" tile (handled by the composer,
		 * which owns the ingest rules and error toasts). */
		onFiles: (files: FileList) => void;
	}

	let { media = $bindable(), onFiles }: Props = $props();

	const showClearAll = $derived(
		media?.kind === 'photos' && media.items.length > 1,
	);

	async function addMore() {
		const files = await pickFiles({ accept: PHOTO_ACCEPT, multiple: true });
		if (files && files.length > 0) onFiles(files);
	}

	function clear() {
		media = undefined;
	}

	function removePhoto(index: number) {
		if (!media || media.kind !== 'photos') return;
		const remaining = media.items.filter((_, i) => i !== index);
		media =
			remaining.length > 0 ? { kind: 'photos', items: remaining } : undefined;
	}
</script>

{#if media}
	<div class="pt-2" data-testid="message-input-media-preview">
		{#if showClearAll}
			<div class="flex justify-end pb-2 pe-2">
				<button
					type="button"
					class="clear-all flex items-center justify-center"
					data-testid="message-input-clear-attachments"
					aria-label={m.removeAllAttachments()}
					onclick={clear}
				>
					<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
				</button>
			</div>
		{/if}
		<div class="flex max-h-[142px] gap-2 overflow-x-auto px-2">
			{#if media.kind === 'photos'}
				{#each media.items as photo, i (photo)}
					<StagedThumb
						onRemove={() => removePhoto(i)}
						removeTestId="message-input-remove-attachment-{i}"
					>
						<img
							use:objectUrl={photo}
							alt={photo.name}
							class="block h-full w-full object-cover"
						/>
					</StagedThumb>
				{/each}
				{#if media.items.length < MAX_STAGED_PHOTOS}
					<button
						type="button"
						class="add-more flex h-[120px] w-[120px] shrink-0 items-center justify-center"
						data-testid="message-input-add-more"
						aria-label={m.addMoreAttachments()}
						onclick={addMore}
					>
						<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
					</button>
				{/if}
			{:else}
				<StagedThumb
					onRemove={clear}
					removeTestId="message-input-remove-attachment-0"
					class="flex flex-col items-center justify-center gap-2 p-2"
				>
					<ExtensionSheet name={media.file.name} />
					<span class="staged-file-name line-clamp-2 text-center break-all"
						>{media.file.name}</span
					>
				</StagedThumb>
			{/if}
		</div>
	</div>
{/if}

<style>
	.clear-all {
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.6;
		transition: opacity 0.15s ease;
		height: auto;
	}
	.clear-all:hover {
		opacity: 1;
	}
	.clear-all :global(wa-icon) {
		width: 20px;
		height: 20px;
	}

	.add-more {
		border-radius: 4px;
		border: 2px dashed var(--k-hairline-color, rgba(128, 128, 128, 0.4));
		background: transparent;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.6;
		transition: opacity 0.15s ease;
	}
	.add-more:hover {
		opacity: 0.9;
	}
	.add-more :global(wa-icon) {
		width: 28px;
		height: 28px;
	}

	.staged-file-name {
		font-size: 11px;
		line-height: 1.3;
		color: var(--k-text-color);
	}
</style>
