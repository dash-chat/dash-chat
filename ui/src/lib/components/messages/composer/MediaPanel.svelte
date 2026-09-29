<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { type LoadFiles, pickMedia } from '$lib/utils/media';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';
	import RecentPhotosStrip from './RecentPhotosStrip.svelte';

	interface Props {
		onPick: (load: LoadFiles) => Promise<void>;
		onPickerOpen: () => void;
	}

	let { onPick, onPickerOpen }: Props = $props();

	async function pick(mode: 'image' | 'document', multiple: boolean) {
		onPickerOpen();
		try {
			await onPick(() => pickMedia(mode, multiple));
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
		<RecentPhotosStrip {onPick} />
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
