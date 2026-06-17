<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Popover, List, ListItem } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/types/media';

	interface Props {
		opened: boolean;
		onFiles: (files: FileList) => void;
		/** CSS selector of the trigger the popover anchors to. */
		target: string;
	}

	let { opened = $bindable(false), onFiles, target }: Props = $props();

	let photoPicker: HTMLInputElement;
	let filePicker: HTMLInputElement;

	function onPhotosPicked() {
		if (photoPicker.files && photoPicker.files.length > 0) {
			onFiles(photoPicker.files);
		}
		photoPicker.value = '';
		opened = false;
	}

	function onFilePicked() {
		if (filePicker.files && filePicker.files.length > 0) {
			onFiles(filePicker.files);
		}
		filePicker.value = '';
		opened = false;
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

<Popover
	{opened}
	{target}
	onBackdropClick={() => (opened = false)}
	data-testid="message-input-attach-menu"
>
	<List nested>
		<ListItem
			link
			chevron={false}
			title={m.attachPhotos()}
			data-testid="message-input-attach-photos"
			onClick={() => {
				opened = false;
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
				opened = false;
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
