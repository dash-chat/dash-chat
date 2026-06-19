<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/utils/media';
	import { isIos, isTauriEnv } from '$lib/utils/environment';
	import { pickFiles, pickNativeFiles } from '$lib/utils/files';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';

	interface Props {
		opened: boolean;
		onFiles: (files: File[]) => void;
	}

	let { opened = $bindable(false), onFiles }: Props = $props();

	// iOS web `<input type=file>` always shows an action sheet (Photo Library /
	// Take Photo / Choose File); the native picker opens the photo library or
	// document browser directly. Android's web input already opens the right
	// picker directly, so it keeps using it.
	async function pick(mode: 'image' | 'document', multiple: boolean) {
		opened = false;
		try {
			let files: File[] | null;
			if (isIos && isTauriEnv()) {
				files = await pickNativeFiles({ mode, multiple });
			} else {
				const accept = mode === 'image' ? PHOTO_ACCEPT : undefined;
				const list = await pickFiles({ accept, multiple });
				files = list ? Array.from(list) : null;
			}
			if (files && files.length > 0) onFiles(files);
		} catch (e) {
			console.error('Failed to pick files', e);
		}
	}
</script>

{#if opened}
	<div
		class="flex gap-5 bg-page-surface px-5 pt-4 pb-safe-4"
		style="justify-content: space-evenly"
		data-testid="message-input-media-panel"
	>
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
{/if}
