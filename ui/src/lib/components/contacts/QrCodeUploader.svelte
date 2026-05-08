<script lang="ts">
	import { onMount } from 'svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { scanQrFromImage } from '$lib/utils/qrcode';
	import { showToast } from '$lib/utils/toasts';

	type SelectImageHandler = (code: string) => void | Promise<void>;

	let {
		autoOpen = true,
		onSelectImage,
	}: {
		autoOpen?: boolean;
		onSelectImage: SelectImageHandler;
	} = $props();

	let imageFilePicker: HTMLInputElement;

	async function onImageSelected() {
		if (!imageFilePicker.files || !imageFilePicker.files[0]) return;
		try {
			const code = await scanQrFromImage(imageFilePicker.files[0]);
			await onSelectImage(code);
		} catch (e) {
			console.error(e);
			showToast(m.errorNoQrCodeInImage(), 'error');
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
