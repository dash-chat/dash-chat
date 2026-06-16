<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/types/media';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';

	interface Props {
		opened: boolean;
		onFiles: (files: FileList) => void;
	}

	let { opened = $bindable(false), onFiles }: Props = $props();

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

{#if opened}
	<div
		class="pb-safe flex gap-5 px-5 pt-4 pb-4"
		data-testid="message-input-media-panel"
	>
		<LabelledIconButton
			label={m.gallery()}
			icon={mdiImage}
			testId="message-input-attach-photos"
			onClick={() => photoPicker.click()}
		/>
		<LabelledIconButton
			label={m.attachFile()}
			icon={mdiFile}
			testId="message-input-attach-file"
			onClick={() => filePicker.click()}
		/>
	</div>
{/if}
