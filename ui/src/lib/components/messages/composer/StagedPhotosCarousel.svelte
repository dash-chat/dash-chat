<script lang="ts">
	import { tick } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiTrashCanOutline, mdiPlusBoxOutline } from '@mdi/js';
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

	let carouselEl: HTMLElement | undefined = $state();

	const photos = $derived(media?.kind === 'photos' ? media.items : []);

	// Keep the selected index in range as photos are added or removed.
	$effect(() => {
		if (index > photos.length - 1) index = Math.max(0, photos.length - 1);
	});

	// Page via scrollIntoView / bounding-box proximity rather than scrollLeft math:
	// RTL's scrollLeft sign convention differs across engines (Chromium/Gecko go
	// negative, WebKit/iOS stays positive), and these are direction-agnostic.
	function scrollToIndex(i: number, smooth = true) {
		const slide = carouselEl?.children[i] as Element | undefined;
		slide?.scrollIntoView({
			behavior: smooth ? 'smooth' : 'auto',
			inline: 'center',
			block: 'nearest',
		});
	}

	function select(i: number) {
		index = Math.max(0, Math.min(photos.length - 1, i));
		scrollToIndex(index, false);
	}

	function onCarouselScroll() {
		if (!carouselEl || carouselEl.clientWidth === 0) return;
		// The active page is the slide whose centre is closest to the viewport's.
		const viewportCenter =
			carouselEl.getBoundingClientRect().left + carouselEl.clientWidth / 2;
		let nearest = index;
		let nearestDistance = Infinity;
		for (let i = 0; i < carouselEl.children.length; i++) {
			const rect = carouselEl.children[i].getBoundingClientRect();
			const distance = Math.abs(rect.left + rect.width / 2 - viewportCenter);
			if (distance < nearestDistance) {
				nearestDistance = distance;
				nearest = i;
			}
		}
		if (nearest !== index && nearest < photos.length) index = nearest;
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
		scrollToIndex(index, false);
	}

	function onKeydown(event: KeyboardEvent) {
		// Don't hijack cursor movement while typing the caption.
		if (event.target instanceof HTMLTextAreaElement) return;
		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			select(index - 1);
		} else if (event.key === 'ArrowRight') {
			event.preventDefault();
			select(index + 1);
		}
	}
</script>

<svelte:window onkeydown={onKeydown} />

<div class="relative flex min-h-0 flex-1 flex-col overflow-hidden">
	<div
		class="carousel flex min-h-0 flex-1 snap-x snap-mandatory overflow-x-auto"
		bind:this={carouselEl}
		onscroll={onCarouselScroll}
	>
		{#each photos as photo (photo)}
			<div
				class="flex w-full shrink-0 snap-center snap-always items-center justify-center px-3 py-3"
			>
				<img
					class="rounded-2xl object-contain"
					style="max-height: 70vh; max-width: 70vw;"
					use:objectUrl={photo}
					alt={photo.name}
				/>
			</div>
		{/each}
	</div>

	<div
		class="absolute inset-x-0 bottom-0 z-10 flex items-center justify-start gap-3 overflow-x-auto px-4 pt-3 pb-3"
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
								icon={mdiTrashCanOutline}
								onClick={() => removePhoto(i)}
								label={m.removeAttachment()}
								testid="staged-media-remove-{i}"
								iconClass="text-xl"
								class="!text-white opacity-100"
							/>
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
					class="!h-10 !w-10 shrink-0 !bg-[#3a3a3c] !opacity-100 hover:!bg-[#4a4a4c]"
				/>
			</div>
		{/if}
	</div>
</div>

<style>
	.carousel {
		scrollbar-width: none;
	}
	.carousel::-webkit-scrollbar {
		display: none;
	}

	.staged-thumb {
		border: 2px solid white;
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.08);
	}
	.staged-thumb.selected {
		border: 2px solid var(--color-brand-primary);
	}
</style>
