<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose, mdiPlus } from '@mdi/js';
	import {
		type DraftMedia,
		MAX_STAGED_PHOTOS,
		PHOTO_ACCEPT,
		revokeDraft,
	} from '$lib/types/media';
	import { stageFiles } from '$lib/utils/stage-files';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';

	interface Props {
		media: DraftMedia | undefined;
	}

	let { media = $bindable() }: Props = $props();

	const count = $derived(media?.kind === 'photos' ? media.items.length : 1);

	let photoPicker: HTMLInputElement;

	function onPhotosPicked() {
		if (!photoPicker.files || photoPicker.files.length === 0) return;
		media = stageFiles(media, photoPicker.files);
		photoPicker.value = '';
	}

	function clear() {
		if (media) revokeDraft(media);
		media = undefined;
	}

	function removePhoto(index: number) {
		if (!media || media.kind !== 'photos') return;
		const removed = media.items[index];
		URL.revokeObjectURL(removed.previewUrl);
		const remaining = media.items.filter((_, i) => i !== index);
		media =
			remaining.length > 0 ? { kind: 'photos', items: remaining } : undefined;
	}
</script>

<input
	type="file"
	accept={PHOTO_ACCEPT}
	multiple
	bind:this={photoPicker}
	class="hidden"
	data-testid="message-input-add-more-picker"
	onchange={onPhotosPicked}
/>

{#if media}
	<div class="pt-2" data-testid="message-input-media-preview">
		{#if count > 1}
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
				{#each media.items as photo, i (photo.previewUrl)}
					<div
						class="staged-thumb relative h-[120px] w-[120px] shrink-0 overflow-hidden"
					>
						<img
							src={photo.previewUrl}
							alt={photo.file.name}
							class="block h-full w-full object-cover"
						/>
						<div
							class="thumb-gradient pointer-events-none absolute inset-x-0 top-0 h-8"
						></div>
						<button
							type="button"
							class="thumb-remove absolute end-1 top-1 flex h-4 w-4 items-center justify-center p-0"
							data-testid="message-input-remove-attachment-{i}"
							aria-label={m.removeAttachment()}
							onclick={() => removePhoto(i)}
						>
							<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
						</button>
					</div>
				{/each}
				{#if media.items.length < MAX_STAGED_PHOTOS}
					<button
						type="button"
						class="add-more flex h-[120px] w-[120px] shrink-0 items-center justify-center"
						data-testid="message-input-add-more"
						aria-label={m.addMoreAttachments()}
						onclick={() => photoPicker.click()}
					>
						<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
					</button>
				{/if}
			{:else}
				<div
					class="staged-thumb relative flex h-[120px] w-[120px] shrink-0 flex-col items-center justify-center gap-2 overflow-hidden p-2"
				>
					<ExtensionSheet name={media.file.name} />
					<span class="staged-file-name line-clamp-2 text-center break-all"
						>{media.file.name}</span
					>
					<div
						class="thumb-gradient pointer-events-none absolute inset-x-0 top-0 h-8"
					></div>
					<button
						type="button"
						class="thumb-remove absolute end-1 top-1 flex h-4 w-4 items-center justify-center p-0"
						data-testid="message-input-remove-attachment-0"
						aria-label={m.removeAttachment()}
						onclick={clear}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
					</button>
				</div>
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
		height: auto
	}
	.clear-all:hover {
		opacity: 1;
	}
	.clear-all :global(wa-icon) {
		width: 20px;
		height: 20px;
	}

	.staged-thumb {
		border-radius: 4px;
		background: rgba(128, 128, 128, 0.1);
	}

	.thumb-gradient {
		background: linear-gradient(rgba(0, 0, 0, 0.4), transparent);
		opacity: 0;
		transition: opacity 0.15s ease;
	}
	.staged-thumb:hover .thumb-gradient {
		opacity: 1;
	}

	.thumb-remove {
		border: none;
		background: transparent;
		cursor: pointer;
		color: white;
	}
	.thumb-remove :global(wa-icon) {
		width: 16px;
		height: 16px;
		filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.6));
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
