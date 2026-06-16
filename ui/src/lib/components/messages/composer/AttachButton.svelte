<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiPlus, mdiImage, mdiFile } from '@mdi/js';
	import { Popover, List, ListItem } from 'konsta/svelte';
	import { PHOTO_ACCEPT } from '$lib/types/media';

	interface Props {
		onFiles: (files: FileList) => void;
	}

	let { onFiles }: Props = $props();

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

<button
	type="button"
	class="attach-button mb-1 flex h-10 w-10 shrink-0 items-center justify-center self-end rounded-full"
	data-testid="message-input-attach"
	aria-label={m.attachMenu()}
	aria-expanded={open}
	onclick={() => (open = !open)}
>
	<wa-icon src={wrapPathInSvg(mdiPlus)}></wa-icon>
</button>

<Popover
	opened={open}
	target="[data-testid='message-input-attach']"
	onBackdropClick={() => (open = false)}
	data-testid="message-input-attach-menu"
>
	<List nested>
		<ListItem
			link
			chevron={false}
			title={m.attachPhotos()}
			data-testid="message-input-attach-photos"
			onClick={() => {
				open = false;
				photoPicker.click();
			}}
		>
			{#snippet media()}
				<wa-icon style="width: 24px; height: 24px" src={wrapPathInSvg(mdiImage)}
				></wa-icon>
			{/snippet}
		</ListItem>
		<ListItem
			link
			chevron={false}
			title={m.attachFile()}
			data-testid="message-input-attach-file"
			onClick={() => {
				open = false;
				filePicker.click();
			}}
		>
			{#snippet media()}
				<wa-icon style="width: 24px; height: 24px" src={wrapPathInSvg(mdiFile)}
				></wa-icon>
			{/snippet}
		</ListItem>
	</List>
</Popover>

<style>
	.attach-button {
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--k-text-color);
		opacity: 0.6;
		transition:
			opacity 0.15s ease,
			background-color 0.15s ease;
	}
	.attach-button:hover {
		opacity: 0.85;
		background: rgba(128, 128, 128, 0.1);
	}
	.attach-button:active {
		background: rgba(128, 128, 128, 0.2);
	}
	.attach-button :global(wa-icon) {
		width: 22px;
		height: 22px;
	}
</style>
