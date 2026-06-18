<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/utils/media';
	import { pickFiles } from '$lib/utils/files';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';

	interface Props {
		opened: boolean;
		onFiles: (files: FileList) => void;
	}

	let { opened = $bindable(false), onFiles }: Props = $props();

	async function pick(accept: string | undefined, multiple: boolean) {
		opened = false;
		const files = await pickFiles({ accept, multiple });
		if (files && files.length > 0) onFiles(files);
	}
</script>

{#if opened}
	<div
		class="flex gap-5 px-5 pt-4 pb-safe-4"
		style="justify-content: space-evenly"
		data-testid="message-input-media-panel"
	>
		<LabelledIconButton
			label={m.gallery()}
			icon={mdiImage}
			testId="message-input-attach-photos"
			onClick={() => pick(PHOTO_ACCEPT, true)}
		/>
		<LabelledIconButton
			label={m.attachFile()}
			icon={mdiFile}
			testId="message-input-attach-file"
			onClick={() => pick(undefined, false)}
		/>
	</div>
{/if}
