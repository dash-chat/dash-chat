<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		mdiLinkVariant,
		mdiTrayArrowDown,
		mdiPalette,
		mdiUpload,
	} from '@mdi/js';
	import { mdiShare } from '$lib/utils/icon';
	import LabelledIconButton from '$lib/components/contacts/LabelledIconButton.svelte';
	import { isMobile } from '$lib/utils/environment';

	type ActionHandler = () => void | Promise<void>;

	let {
		onLink,
		onShare,
		onSave,
		onUpload,
		onOpenColorPicker,
	}: {
		onLink: ActionHandler;
		onShare: ActionHandler;
		onSave: ActionHandler;
		onUpload: ActionHandler;
		onOpenColorPicker: ActionHandler;
	} = $props();
</script>

<div class="row gap-4" style="justify-content: center;">
	{#if isMobile}
		<LabelledIconButton
			label={m.link()}
			icon={mdiLinkVariant}
			onClick={onLink}
			testId="add-contact-link-btn"
		/>
		<LabelledIconButton
			label={m.share()}
			icon={mdiShare}
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
