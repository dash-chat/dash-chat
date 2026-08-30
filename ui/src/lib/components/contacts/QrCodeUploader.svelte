<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { isScanQrFromImageError, scanQrFromImage } from '$lib/utils/qrcode';
	import { showToast } from '$lib/utils/toasts';

	type SelectImageHandler = (code: string) => void | Promise<void>;

	let {
		onSelectImage,
	}: {
		onSelectImage: SelectImageHandler;
	} = $props();

	let imageFilePicker: HTMLInputElement;

	async function onImageSelected() {
		if (!imageFilePicker.files || !imageFilePicker.files[0]) return;
		try {
			const qrCodeValue = await scanQrFromImage(imageFilePicker.files[0]);
			await onSelectImage(qrCodeValue);
		} catch (e) {
			if (isScanQrFromImageError(e) && e.kind === 'NoQrCodeFound') {
				showToast(m.errorNoQrCodeInImage(), 'error');
			} else {
				console.error(e);
				showToast(m.errorUnexpected(), 'unexpected', e);
			}
		} finally {
			if (imageFilePicker) imageFilePicker.value = '';
		}
	}

	export function trigger() {
		imageFilePicker.click();
	}
</script>

<input
	type="file"
	accept="image/*"
	bind:this={imageFilePicker}
	class="hidden"
	data-testid="add-contact-file-input"
	onchange={onImageSelected}
/>
