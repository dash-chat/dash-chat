<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import { m } from '$lib/paraglide/messages';
	import { objectUrl } from '$lib/actions/object-url';
	import { compressImage } from '$lib/utils/compress';
	import { formatFileSize, isVisualFile, pickMedia } from '$lib/utils/media';
	import { showToast } from '$lib/utils/toasts';
	import StagedThumb from '$lib/components/StagedThumb.svelte';
	import AddThumb from '$lib/components/AddThumb.svelte';

	interface Props {
		value: File[];
		picking: boolean;
		max: number;
		maxBytes: number;
		disabled?: boolean;
		testId: string;
	}

	let {
		value = $bindable([]),
		picking = $bindable(false),
		max,
		maxBytes,
		disabled = false,
		testId,
	}: Props = $props();

	let addThumb = $state<AddThumb>();
	let destroyed = false;
	onDestroy(() => {
		destroyed = true;
	});

	async function addImages() {
		picking = true;
		try {
			// One past the free slots, just enough to tell the user they picked too many.
			const picked = await pickMedia('image', true, max - value.length + 1);
			if (picked) await attach(picked);
		} finally {
			picking = false;
		}
		await tick();
		addThumb?.focus();
	}

	/** Compressed one at a time so only a single decoded bitmap is held; support
	 * is judged afterwards, since compressImage turns a decodable HEIC into a JPEG. */
	async function attach(picked: File[]) {
		const room = max - value.length;
		const tooMany = picked.length > room;
		let unsupported = false;
		let tooLarge = false;
		for (const file of picked.slice(0, room)) {
			const image = await compressImage(file);
			if (destroyed) return;
			if (!isVisualFile(image)) {
				unsupported = true;
			} else if (totalBytes(value) + image.size > maxBytes) {
				tooLarge = true;
			} else {
				value = [...value, image];
			}
		}
		// The toast manager shows one at a time, so only the first reason is told.
		if (unsupported) {
			showToast(m.errorUnsupportedImage(), 'error');
		} else if (tooLarge) {
			showToast(
				m.errorAttachmentTooLarge({ max: formatFileSize(maxBytes) }),
				'error',
			);
		} else if (tooMany) {
			showToast(m.errorTooManyAttachments(), 'error');
		}
	}

	function totalBytes(files: File[]): number {
		return files.reduce((sum, file) => sum + file.size, 0);
	}

	async function remove(index: number) {
		value = value.filter((_, i) => i !== index);
		await tick();
		addThumb?.focus();
	}
</script>

<div
	class="grid grid-cols-3 gap-2"
	class:opacity-60={disabled}
	inert={disabled || picking}
	data-testid={testId}
>
	{#each value as image, i (image)}
		<StagedThumb
			removeLabel={m.removeNamedAttachment({ name: image.name })}
			onRemove={() => remove(i)}
			removeTestId="{testId}-remove-{i}"
			class="aspect-square w-full"
		>
			<img
				use:objectUrl={image}
				alt={image.name}
				class="block h-full w-full object-cover"
			/>
		</StagedThumb>
	{/each}
	{#if value.length < max}
		<AddThumb
			bind:this={addThumb}
			onClick={addImages}
			label={m.selectImage()}
			testId="{testId}-add"
			disabled={picking}
			class="aspect-square w-full"
		/>
	{/if}
</div>
