<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { Popover, List, ListItem } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/utils/media';
	import { pickFiles } from '$lib/utils/files';

	interface Props {
		opened: boolean;
		onFiles: (files: FileList) => void;
		/** CSS selector of the trigger the popover anchors to. */
		target: string;
	}

	let { opened = $bindable(false), onFiles, target }: Props = $props();

	async function pick(accept: string | undefined, multiple: boolean) {
		opened = false;
		const files = await pickFiles({ accept, multiple });
		if (files && files.length > 0) onFiles(files);
	}
</script>

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
			onClick={() => pick(PHOTO_ACCEPT, true)}
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
			onClick={() => pick(undefined, false)}
		>
			{#snippet media()}
				<wa-icon style="width: 24px; height: 24px" src={wrapPathInSvg(mdiFile)}
				></wa-icon>
			{/snippet}
		</ListItem>
	</List>
</Popover>
