<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiImageSearchOutline } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import { scanQrCode } from '$lib/utils/qrcode';
	import { showToast } from '$lib/utils/toasts';
	import { isTauriEnv } from '$lib/utils/environment';
	import QrCodeUploader from './QrCodeUploader.svelte';

	type SelectImageHandler = (code: string) => void | Promise<void>;

	let {
		onSelectImage,
	}: {
		onSelectImage: SelectImageHandler;
	} = $props();

	let uploaderRef: QrCodeUploader | null = $state(null);

	let cancelled = false;

	export async function cancelScanner() {
		if (cancelled) return;
		cancelled = true;

		if (isTauriEnv()) {
			const { cancel } = await import('@tauri-apps/plugin-barcode-scanner');
			await cancel();
		}
	}

	onMount(async () => {
		cancelled = false;
		try {
			const qrCodeValue = await scanQrCode();
			await onSelectImage(qrCodeValue);
		} catch (e) {
			if (cancelled) return;
			console.error(e);
			showToast(m.errorScanningQrCode(), 'error');
		}
	});

	onDestroy(() => {
		// Fire and forget, we don't want to await this and block the UI from closing
		cancelScanner();
	});
</script>

<div class="column" style="position: relative; flex: 1;">
	<div
		class="row p-4"
		style="color: white; align-items: center; justify-content: center; z-index: 1; text-align: center"
	>
		<span class="w-60">{m.scanQrCodeOfYourContact()}</span>
	</div>
	<div
		class="column"
		style="flex: 1; align-items: center; justify-content: center"
	>
		<div class="barcode-scanner--area--container">
			<div class="square surround-cover">
				<div class="barcode-scanner--area--outer surround-cover"></div>
			</div>
		</div>
	</div>
	<div
		style="padding-bottom: 24px; padding-top: 24px; display: flex; justify-content: center; z-index: 1;"
	>
		<button
			class="w-14 h-14 rounded-full bg-white text-gray-700 border-none cursor-pointer flex items-center justify-center shadow-[0_2px_8px_rgba(0,0,0,0.3)] transition-transform duration-200 hover:scale-105 active:scale-95"
			onclick={() => uploaderRef?.trigger()}
			aria-label={m.photo()}
			data-testid="add-contact-select-image-btn"
		>
			<wa-icon
				src={wrapPathInSvg(mdiImageSearchOutline)}
				style="font-size: 28px"
			></wa-icon>
		</button>
	</div>

	<QrCodeUploader {onSelectImage} bind:this={uploaderRef} />
</div>

<style>
	.square {
		width: 100%;
		position: relative;
		overflow: hidden;
		transition: 0.3s;
	}
	.square:after {
		content: '';
		top: 0;
		display: block;
		padding-bottom: 100%;
	}
	.square > div {
		position: absolute;
		inset: 0;
	}

	.surround-cover {
		box-shadow: 0 0 0 99999px rgba(0, 0, 0, 0.5);
	}

	.barcode-scanner--area--container {
		width: 80%;
		max-width: min(500px, 80vh);
	}
	.barcode-scanner--area--outer {
		display: flex;
		border-radius: 1em;
	}
</style>
