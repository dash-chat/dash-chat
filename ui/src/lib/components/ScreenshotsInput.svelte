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
		attaching: boolean;
		max: number;
		maxBytes: number;
		disabled?: boolean;
		labelledBy: string;
		testId: string;
	}

	let {
		value = $bindable([]),
		attaching = $bindable(false),
		max,
		maxBytes,
		disabled = false,
		labelledBy,
		testId,
	}: Props = $props();

	let grid = $state<HTMLDivElement>();
	let pickerOpen = $state(false);
	let destroyed = false;
	onDestroy(() => {
		destroyed = true;
	});

	// Only the compression phase locks the form: a picker whose dismissal the
	// webview never reports would otherwise leave it locked for good.
	async function addImages() {
		const hadFocus = grid?.contains(document.activeElement) ?? false;
		pickerOpen = true;
		let picked: File[] | null;
		try {
			// One past the free slots, just enough to tell the user they picked too many.
			picked = await pickMedia('image', true, max - value.length + 1);
		} finally {
			pickerOpen = false;
		}
		if (!picked) return;
		attaching = true;
		try {
			await attach(picked);
		} finally {
			attaching = false;
		}
		if (!hadFocus) return;
		await tick();
		focusLastControl();
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
				break;
			} else {
				value = [...value, image];
			}
		}
		// The toast manager shows one at a time, so only the first reason is told.
		if (unsupported) {
			showToast(m.errorUnsupportedImage(), 'error');
		} else if (tooLarge) {
			showToast(
				m.errorScreenshotsTooLarge({ max: formatFileSize(maxBytes) }),
				'error',
			);
		} else if (tooMany) {
			showToast(m.errorTooManyScreenshots(), 'error');
		}
	}

	function totalBytes(files: File[]): number {
		return files.reduce((sum, file) => sum + file.size, 0);
	}

	async function remove(index: number) {
		const hadFocus = grid?.contains(document.activeElement) ?? false;
		value = value.filter((_, i) => i !== index);
		if (!hadFocus) return;
		await tick();
		focusLastControl();
	}

	// Relies on each tile rendering one button, with the add tile last.
	function focusLastControl() {
		const controls = grid?.querySelectorAll('button') ?? [];
		[...controls].at(-1)?.focus();
	}
</script>

<div
	bind:this={grid}
	role="group"
	aria-labelledby={labelledBy}
	class="grid grid-cols-3 gap-2"
	class:opacity-60={disabled}
	inert={disabled || attaching}
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
			onClick={addImages}
			label={m.addImage()}
			testId="{testId}-add"
			disabled={pickerOpen || attaching}
			class="aspect-square w-full"
		/>
	{/if}
</div>
