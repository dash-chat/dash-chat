<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/dropdown/dropdown.js';
	import '@awesome.me/webawesome/dist/components/dropdown-item/dropdown-item.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiImage, mdiFile } from '@mdi/js';
	import { PHOTO_ACCEPT } from '$lib/utils/media';
	import { pickFiles } from '$lib/utils/files';
	import AttachButton from './AttachButton.svelte';

	interface Props {
		onFiles: (files: FileList) => void;
	}

	let { onFiles }: Props = $props();

	let open = $state(false);

	async function pick(accept: string | undefined, multiple: boolean) {
		const files = await pickFiles({ accept, multiple });
		if (files && files.length > 0) onFiles(files);
	}

	function onSelect(event: CustomEvent<{ item: { value: string } }>) {
		if (event.detail.item.value === 'photos') pick(PHOTO_ACCEPT, true);
		else if (event.detail.item.value === 'file') pick(undefined, false);
	}
</script>

<wa-dropdown
	class="contents"
	placement="top-start"
	distance={8}
	data-testid="message-input-attach-menu"
	onwa-select={onSelect}
	onwa-show={() => (open = true)}
	onwa-after-hide={() => (open = false)}
>
	<div slot="trigger" class="inline-flex">
		<AttachButton class="h-10 w-10" expanded={open} />
	</div>
	<wa-dropdown-item value="photos" data-testid="message-input-attach-photos">
		<wa-icon slot="icon" src={wrapPathInSvg(mdiImage)}></wa-icon>
		{m.attachPhotos()}
	</wa-dropdown-item>
	<wa-dropdown-item value="file" data-testid="message-input-attach-file">
		<wa-icon slot="icon" src={wrapPathInSvg(mdiFile)}></wa-icon>
		{m.attachFile()}
	</wa-dropdown-item>
</wa-dropdown>

<style>
	/* Tailwind's preflight resets `padding: 0` on every element, which beats
	 * WebAwesome's `:host { padding }` and collapses the dropdown items to the
	 * height of their text. Re-apply the component's intended item padding. */
	:global(wa-dropdown-item) {
		padding-left: 0.5em;
		padding-top: 0.5em;
		padding-bottom: 0.5em;
		padding-right: 1em;
	}

	:global(wa-dropdown-item::part(icon)) {
		width: 42px;
	}
</style>
