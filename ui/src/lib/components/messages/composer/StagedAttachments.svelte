<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiClose } from '@mdi/js';
	import { MAX_STAGED_PHOTOS } from '$lib/utils/media';
	import type { MediaDraft } from '$lib/components/messages/composer/media-draft.svelte';
	import ExtensionSheet from '$lib/components/ExtensionSheet.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import StagedThumb from '$lib/components/StagedThumb.svelte';
	import AddThumb from '$lib/components/AddThumb.svelte';
	import { objectUrl } from '$lib/actions/object-url';

	interface Props {
		draft: MediaDraft;
		loadingMedia: boolean;
		onAddMore: () => void;
	}

	let { draft, loadingMedia, onAddMore }: Props = $props();

	const media = $derived(draft.media);

	const showClearAll = $derived(
		media?.kind === 'photos' && media.items.length > 1,
	);
</script>

{#if media && media.kind !== 'voice_note'}
	<div class="pt-2" data-testid="message-input-media-preview">
		{#if showClearAll}
			<div class="flex justify-end pb-2 pe-2">
				<IconButton
					onClick={() => draft.clear()}
					label={m.removeAllAttachments()}
					testid="message-input-clear-attachments"
					class="!h-7 !w-7"
				>
					<wa-icon class="text-xl" src={wrapPathInSvg(mdiClose)}></wa-icon>
				</IconButton>
			</div>
		{/if}
		<div class="flex max-h-[142px] gap-2 overflow-x-auto px-2">
			{#if media.kind === 'photos'}
				{#each media.items as photo, i (photo)}
					<StagedThumb
						removeLabel={m.removeAttachment()}
						onRemove={() => draft.removePhoto(i)}
						removeTestId="message-input-remove-attachment-{i}"
						class="h-[120px] w-[120px]"
					>
						<img
							use:objectUrl={{ data: photo.data, mimeType: photo.mime_type }}
							alt={photo.name}
							class="block h-full w-full object-cover"
						/>
					</StagedThumb>
				{/each}
				{#if media.items.length < MAX_STAGED_PHOTOS}
					<AddThumb
						onClick={onAddMore}
						label={m.addMoreAttachments()}
						testId="message-input-add-more"
						disabled={loadingMedia}
						class="h-[120px] w-[120px]"
					/>
				{/if}
			{:else}
				<StagedThumb
					removeLabel={m.removeAttachment()}
					onRemove={() => draft.clear()}
					removeTestId="message-input-remove-attachment-0"
					class="flex h-[120px] w-[120px] flex-col items-center justify-center gap-2 p-2"
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
	.staged-file-name {
		font-size: 11px;
		line-height: 1.3;
		color: var(--k-text-color);
	}
</style>
