<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPlus, mdiImage, mdiFile } from '@mdi/js';
	import { useTheme } from 'konsta/svelte';
	import { PHOTO_ACCEPT } from '$lib/types/media';

	interface Props {
		onFiles: (files: FileList) => void;
	}

	let { onFiles }: Props = $props();

	const theme = $derived(useTheme());
	let open = $state(false);
	let photoPicker: HTMLInputElement;
	let filePicker: HTMLInputElement;

	function onPhotosPicked() {
		if (!photoPicker.files || photoPicker.files.length === 0) return;
		onFiles(photoPicker.files);
		photoPicker.value = '';
		open = false;
	}

	function onFilePicked() {
		if (!filePicker.files || !filePicker.files[0]) return;
		onFiles(filePicker.files);
		filePicker.value = '';
		open = false;
	}
</script>

<input
	type="file"
	accept={PHOTO_ACCEPT}
	multiple
	bind:this={photoPicker}
	class="hidden"
	data-testid="message-input-photo-picker"
	onchange={onPhotosPicked}
/>
<input
	type="file"
	bind:this={filePicker}
	class="hidden"
	data-testid="message-input-file-picker"
	onchange={onFilePicked}
/>

<div class="relative mb-1 self-end">
	<button
		type="button"
		class="icon-button attach-button flex h-10 w-10 shrink-0 items-center justify-center"
		data-testid="message-input-attach"
		aria-label={m.attachMenu()}
		aria-expanded={open}
		onclick={() => (open = !open)}
	>
		<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
	</button>
	{#if open}
		<button
			type="button"
			class="fixed inset-0 z-10 cursor-default border-none bg-transparent"
			aria-label={m.close()}
			onclick={() => (open = false)}
		></button>
		<div
			class="attach-menu absolute bottom-[calc(100%_+_8px)] start-0 z-20 min-w-[200px] py-1.5"
			class:attach-menu-ios={theme === 'ios'}
			data-testid="message-input-attach-menu"
		>
			<button
				type="button"
				class="attach-menu-item flex w-full items-center gap-3 px-4 py-2.5"
				data-testid="message-input-attach-photos"
				onclick={() => {
					open = false;
					photoPicker.click();
				}}
			>
				<wa-icon src={wrapPathInSvg(mdiImage)}></wa-icon>
				<span>{m.attachPhotos()}</span>
			</button>
			<button
				type="button"
				class="attach-menu-item flex w-full items-center gap-3 px-4 py-2.5"
				data-testid="message-input-attach-file"
				onclick={() => {
					open = false;
					filePicker.click();
				}}
			>
				<wa-icon src={wrapPathInSvg(mdiFile)}></wa-icon>
				<span>{m.attachFile()}</span>
			</button>
		</div>
	{/if}
</div>

<style>
	.icon-button {
		border: none;
		background: transparent;
		border-radius: 50%;
		cursor: pointer;
		color: var(--k-text-color);
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}

	.icon-button:active {
		background: rgba(128, 128, 128, 0.2);
	}

	.icon-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}

	.attach-button {
		opacity: 0.6;
	}
	.attach-button:hover {
		opacity: 0.85;
		background: rgba(128, 128, 128, 0.1);
	}

	.attach-menu {
		border-radius: 14px;
		background: var(--k-bars-bg-color, white);
		border: 1px solid var(--k-hairline-color);
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
	}
	:global(.dark) .attach-menu {
		background: var(--k-bars-bg-color, #1c1c1e);
	}

	.attach-menu-item {
		border: none;
		background: transparent;
		cursor: pointer;
		font-size: 15px;
		color: var(--k-text-color);
		transition: background-color 0.1s ease;
	}

	.attach-menu-item:hover {
		background: rgba(128, 128, 128, 0.12);
	}

	.attach-menu-item :global(wa-icon) {
		width: 22px;
		height: 22px;
		opacity: 0.75;
	}
</style>
