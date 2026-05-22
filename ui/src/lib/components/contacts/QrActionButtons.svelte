<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiShareVariant,
		mdiTrayArrowDown,
		mdiPalette,
		mdiUpload,
	} from '@mdi/js';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';

	type ActionHandler = () => void | Promise<void>;

	let {
		isMobile,
		onShare,
		onSave,
		onUpload,
		onOpenColorPicker,
	}: {
		isMobile: boolean;
		onShare: ActionHandler;
		onSave: ActionHandler;
		onUpload: ActionHandler;
		onOpenColorPicker: ActionHandler;
	} = $props();
</script>

<div class="row gap-4" style="justify-content: center;">
	{#if isMobile}
		<LabelledIconButton
			label={m.share()}
			icon={mdiShareVariant}
			onClick={onShare}
			testId="add-contact-share-btn"
		/>
	{:else}
		<LabelledIconButton
			label={m.save()}
			icon={mdiTrayArrowDown}
			onClick={onSave}
			testId="add-contact-save-btn"
		/>
		<LabelledIconButton
			label={m.upload()}
			icon={mdiUpload}
			onClick={onUpload}
			testId="add-contact-upload-btn"
		/>
	{/if}

	<LabelledIconButton
		label={m.color()}
		icon={mdiPalette}
		onClick={onOpenColorPicker}
		testId="add-contact-color-btn"
	/>
</div>
