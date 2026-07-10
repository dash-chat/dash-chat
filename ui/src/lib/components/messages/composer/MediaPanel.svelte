<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { pickMedia } from '$lib/utils/media';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';
	import RecentPhotosStrip from './RecentPhotosStrip.svelte';
	import { renderBelowKeyboard } from './render-below-keyboard.svelte';

	interface Props {
		/** Whether the panel is open. Bindable: the action also clears it once the
		 *  keyboard reclaims the slot, and the panel closes itself on file pick. */
		open?: boolean;
		onFiles: (files: File[]) => void;
	}

	let { open = $bindable(false), onFiles }: Props = $props();

	function selectFiles(files: File[]) {
		open = false;
		onFiles(files);
	}

	async function pick(mode: 'image' | 'document', multiple: boolean) {
		open = false;
		try {
			const files = await pickMedia(mode, multiple);
			if (files && files.length > 0) onFiles(files);
		} catch (e) {
			console.error('Failed to pick files', e);
		}
	}
</script>

<div
	class="media-panel bg-page-surface"
	use:renderBelowKeyboard={{ open, onClose: () => (open = false) }}
>
	{#if open}
		<div
			class="flex h-full flex-col pt-3 pb-safe-2"
			data-testid="message-input-media-panel"
		>
			<div class="min-h-0 flex-1">
				<RecentPhotosStrip onFiles={selectFiles} />
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
	{/if}
</div>

<style>
	.media-panel {
		overflow: hidden;
	}
</style>
