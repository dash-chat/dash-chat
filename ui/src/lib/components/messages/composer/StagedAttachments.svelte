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
	<div class="staged-attachments" data-testid="message-input-media-preview">
		{#if count > 1}
			<div class="staged-header">
				<button
					type="button"
					class="clear-all"
					data-testid="message-input-clear-attachments"
					aria-label={m.removeAllAttachments()}
					onclick={clear}
				>
					<wa-icon src={wrapPathInSvg(mdiClose)}></wa-icon>
				</button>
			</div>
		{/if}
		<div class="staged-rail">
			{#if media.kind === 'photos'}
				{#each media.items as photo, i (photo.previewUrl)}
					<div class="staged-thumb">
						<img src={photo.previewUrl} alt={photo.file.name} />
						<div class="thumb-gradient"></div>
						<button
							type="button"
							class="thumb-remove"
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
						class="add-more"
						data-testid="message-input-add-more"
						aria-label={m.addMoreAttachments()}
						onclick={() => photoPicker.click()}
					>
						<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
					</button>
				{/if}
			{:else}
				<div class="staged-thumb staged-file">
					<ExtensionSheet name={media.file.name} />
					<span class="staged-file-name">{media.file.name}</span>
					<div class="thumb-gradient"></div>
					<button
						type="button"
						class="thumb-remove"
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
	.staged-attachments {
		padding: 8px 8px 4px;
	}

	.staged-header {
		display: flex;
		justify-content: flex-end;
		padding-bottom: 4px;
	}

	.clear-all {
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: transparent;
		cursor: pointer;
		padding: 2px;
		color: var(--k-text-color);
		opacity: 0.6;
		transition: opacity 0.15s ease;
	}
	.clear-all:hover {
		opacity: 1;
	}
	.clear-all :global(wa-icon) {
		width: 20px;
		height: 20px;
	}

	.staged-rail {
		display: flex;
		gap: 8px;
		overflow-x: auto;
		max-height: 142px;
		padding-bottom: 4px;
	}

	.staged-thumb {
		position: relative;
		flex-shrink: 0;
		width: 120px;
		height: 120px;
		border-radius: 4px;
		overflow: hidden;
		background: rgba(128, 128, 128, 0.1);
	}

	.staged-thumb img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.thumb-gradient {
		position: absolute;
		inset-inline: 0;
		top: 0;
		height: 32px;
		background: linear-gradient(rgba(0, 0, 0, 0.4), transparent);
		opacity: 0;
		transition: opacity 0.15s ease;
		pointer-events: none;
	}

	.staged-thumb:hover .thumb-gradient {
		opacity: 1;
	}

	.thumb-remove {
		position: absolute;
		top: 4px;
		inset-inline-end: 4px;
		width: 16px;
		height: 16px;
		border: none;
		background: transparent;
		padding: 0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
	}

	.thumb-remove :global(wa-icon) {
		width: 16px;
		height: 16px;
		filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.6));
	}

	.add-more {
		flex-shrink: 0;
		width: 120px;
		height: 120px;
		border-radius: 4px;
		border: 2px dashed var(--k-hairline-color, rgba(128, 128, 128, 0.4));
		background: transparent;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
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

	.staged-file {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 8px;
	}

	.staged-file-name {
		font-size: 11px;
		line-height: 1.3;
		text-align: center;
		color: var(--k-text-color);
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		word-break: break-all;
	}
</style>
