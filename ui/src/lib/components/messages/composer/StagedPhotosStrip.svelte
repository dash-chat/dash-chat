<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { tick } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiTrashCanOutline, mdiPlusBoxOutline } from '@mdi/js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { type DraftMedia, MAX_STAGED_PHOTOS } from '$lib/utils/media';
	import { objectUrl } from '$lib/actions/object-url';
	import IconButton from '$lib/components/IconButton.svelte';

	interface Props {
		/** The staged draft. Only the `photos` variant is rendered. */
		media: DraftMedia | undefined;
		/** Index of the currently shown photo. */
		index?: number;
		onAddMore: () => void;
		/** Called when the last photo is removed and nothing remains to show. */
		onClose: () => void;
	}

	let {
		media = $bindable(),
		index = $bindable(0),
		onAddMore,
		onClose,
	}: Props = $props();

	const photos = $derived(media?.kind === 'photos' ? media.items : []);

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
	}

	async function removePhoto(i: number) {
		if (media?.kind !== 'photos') return;
		const remaining = media.items.filter((_, j) => j !== i);
		if (remaining.length === 0) {
			onClose();
			return;
		}
		media = { kind: 'photos', items: remaining };
		await tick();
		index = Math.min(index, remaining.length - 1);
	}
</script>

<div
	class="flex items-center justify-start gap-3 overflow-x-auto px-4 pt-3 pb-3"
	data-testid="staged-media-strip"
>
	{#if photos.length > 1}
		{#each photos as photo, i (photo)}
			<div
				class="staged-thumb relative h-14 w-14 shrink-0 overflow-hidden"
				class:selected={i === index}
			>
				<button
					type="button"
					class="block h-full w-full p-0"
					data-testid="staged-media-thumb-{i}"
					aria-label={photo.name}
					onclick={() => select(i)}
				>
					<img
						use:objectUrl={photo}
						alt={photo.name}
						class="block h-full w-full object-cover"
					/>
				</button>
				{#if i === index}
					<div
						class="absolute inset-0 flex items-center justify-center bg-black/45"
					>
						<IconButton
							onClick={() => removePhoto(i)}
							label={m.removeAttachment()}
							testid="staged-media-remove-{i}"
							class="!text-white opacity-100"
						>
							<wa-icon class="text-xl" src={wrapPathInSvg(mdiTrashCanOutline)}
							></wa-icon>
						</IconButton>
					</div>
				{/if}
			</div>
		{/each}
	{/if}
	{#if photos.length < MAX_STAGED_PHOTOS}
		<div class="flex h-14 shrink-0 items-center">
			<IconButton
				icon={mdiPlusBoxOutline}
				onClick={onAddMore}
				label={m.addMoreAttachments()}
				testid="staged-media-add-more"
				class="shrink-0 !bg-[#3a3a3c] !opacity-100 hover:!bg-[#4a4a4c]"
			/>
		</div>
	{/if}
</div>

<style>
	.staged-thumb {
		border: 2px solid white;
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.08);
	}
	.staged-thumb.selected {
		border: 2px solid var(--color-brand-primary);
	}
</style>
