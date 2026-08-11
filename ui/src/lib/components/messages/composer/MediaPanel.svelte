<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { pickMedia } from '$lib/utils/media';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';
	import RecentPhotosStrip from './RecentPhotosStrip.svelte';

	interface Props {
		onFiles: (files: File[]) => void;
		onPickerOpen: () => void;
	}

	let { onFiles, onPickerOpen }: Props = $props();

	async function pick(mode: 'image' | 'document', multiple: boolean) {
		onPickerOpen();
		try {
			const files = await pickMedia(mode, multiple);
			if (files && files.length > 0) onFiles(files);
		} catch (e) {
			console.error('Failed to pick files', e);
		}
	}
</script>

<div
	class="flex h-full flex-col pt-3 pb-safe-2"
	data-testid="message-input-media-panel"
>
	<div class="min-h-0 flex-1">
		<RecentPhotosStrip {onFiles} />
	</div>
	<div class="flex gap-5 px-5 pt-1" style="justify-content: space-evenly">
		<LabelledIconButton
			label={m.gallery()}
			icon={mdiImage}
			testId="message-input-attach-photos"
			onClick={() => pick('image', true)}
		/>
		<LabelledIconButton
			label={m.attachFile()}
			icon={mdiFile}
			testId="message-input-attach-file"
			onClick={() => pick('document', false)}
		/>
	</div>
</div>
