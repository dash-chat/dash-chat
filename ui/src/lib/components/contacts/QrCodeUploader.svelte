<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isScanQrFromImageError, scanQrFromImage } from '$lib/utils/qrcode';
	import { showToast } from '$lib/utils/toasts';

	type SelectImageHandler = (code: string) => void | Promise<void>;
	type ErrorHandler = (error: unknown) => void | Promise<void>;

	let {
		autoOpen = true,
		onError,
		onSelectImage,
	}: {
		autoOpen?: boolean;
		onError?: ErrorHandler;
		onSelectImage: SelectImageHandler;
	} = $props();

	let imageFilePicker: HTMLInputElement;

	async function onImageSelected() {
		if (!imageFilePicker.files || !imageFilePicker.files[0]) return;
		try {
			const code = await scanQrFromImage(imageFilePicker.files[0]);
			await onSelectImage(code);
		} catch (e) {
			if (isScanQrFromImageError(e) && e.kind === 'NoQrCodeFound') {
				showToast(m.errorNoQrCodeInImage(), 'error');
			} else {
				console.error(e);
				showToast(m.errorUnexpected(), 'unexpected', e);
			}

			await onError?.(e);
		} finally {
			imageFilePicker.value = '';
		}
	}

	export function trigger() {
		imageFilePicker.click();
	}

	onMount(() => {
		if (!autoOpen) return;
		trigger();
	});
</script>

<input
	type="file"
	accept="image/*"
	bind:this={imageFilePicker}
	style="display: none"
	onchange={onImageSelected}
/>
