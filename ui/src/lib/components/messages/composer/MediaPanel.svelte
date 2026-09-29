<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';
	import type { RecentPhoto } from '$lib/utils/recent-photos';
	import RecentPhotosStrip from './RecentPhotosStrip.svelte';

	interface Props {
		loadingMedia: boolean;
		onPickPhotos: () => void;
		onPickFile: () => void;
		onAddRecent: (photo: RecentPhoto) => Promise<void>;
		onPickerOpen: () => void;
	}

	let {
		loadingMedia,
		onPickPhotos,
		onPickFile,
		onAddRecent,
		onPickerOpen,
	}: Props = $props();

	function pick(onPick: () => void) {
		onPickerOpen();
		onPick();
	}
</script>

<div
	class="flex h-full flex-col pt-3 pb-safe-2"
	data-testid="message-input-media-panel"
>
	<div class="min-h-0 flex-1">
		<RecentPhotosStrip {loadingMedia} onAdd={onAddRecent} />
	</div>
	<div class="flex gap-5 px-5 pt-1" style="justify-content: space-evenly">
		<LabelledIconButton
			label={m.gallery()}
			icon={mdiImage}
			testId="message-input-attach-photos"
			disabled={loadingMedia}
			onClick={() => pick(onPickPhotos)}
		/>
		<LabelledIconButton
			label={m.attachFile()}
			icon={mdiFile}
			testId="message-input-attach-file"
			disabled={loadingMedia}
			onClick={() => pick(onPickFile)}
		/>
	</div>
</div>
